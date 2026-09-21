//! Exercises native verification through the HTTP MRTR tool-input envelope.

use super::*;
use codex_protocol::mcp::OPENAI_ELICITATION_EXTENSION_ID;
use codex_rmcp_client::SendElicitation;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn native_request() -> Value {
    json!({
        "method": "openai/elicitation/create",
        "params": {
            "mode": "openai/userVerification", "title": "Verify action",
            "description": "Verify the requested operation", "challenge": "AQID",
            "_meta": {"example/display": {"label": "Operation"}}
        }
    })
}

fn rich_form_request() -> Value {
    let mut request = elicitation_request("form");
    request["method"] = json!("openai/elicitation/create");
    request
}

fn input_required(request: Value) -> Value {
    json!({
        "resultType": "input_required", "content": [], "isError": false,
        "inputRequests": {"verification": request}, "requestState": OPAQUE_STATE
    })
}

fn capabilities() -> ClientCapabilities {
    let mut capabilities = ClientCapabilities::default();
    capabilities.extensions = Some(
        [(
            OPENAI_ELICITATION_EXTENSION_ID.into(),
            serde_json::Map::from_iter([("userVerification".into(), json!({}))]),
        )]
        .into_iter()
        .collect(),
    );
    capabilities
}

async fn client(
    server: &MockServer,
    capabilities: ClientCapabilities,
    handler: SendElicitation,
) -> anyhow::Result<Arc<RmcpClient>> {
    let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
        "native-mrtr-test",
        &format!("{}/mcp", server.uri()),
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
        McpProtocolMode::V20260728,
    )
    .await?;
    client
        .initialize(
            InitializeRequestParams::new(capabilities, Implementation::new("test", "0.0.0")),
            Some(Duration::from_secs(/*secs*/ 5)),
            handler,
        )
        .await?;
    Ok(Arc::new(client))
}

async fn call(client: &RmcpClient) -> anyhow::Result<Value> {
    Ok(serde_json::to_value(
        client
            .call_tool(
                "verified_operation".into(),
                Some(json!({"resource": "example"})),
                Some(json!({"caller": "preserved"})),
                Some(Duration::from_secs(/*secs*/ 5)),
            )
            .await?,
    )?)
}

fn response(action: ElicitationAction, content: Value) -> ElicitationResponse {
    ElicitationResponse {
        action,
        content: Some(content),
        meta: Some(json!({"discard": true})),
    }
}

#[tokio::test]
async fn native_mrtr_returns_validated_proof_or_cancellation_in_content() -> anyhow::Result<()> {
    let proof = json!({"credentialId": "test-credential", "signature": "BAUG"});
    for (action, content, expected) in [
        (
            ElicitationAction::Accept,
            proof.clone(),
            json!({"action": "accept", "content": proof}),
        ),
        (
            ElicitationAction::Cancel,
            proof.clone(),
            json!({"action": "cancel"}),
        ),
        (
            ElicitationAction::Decline,
            proof,
            json!({"action": "decline"}),
        ),
        (
            ElicitationAction::Accept,
            json!({"signature": "invalid"}),
            json!({"action": "cancel"}),
        ),
    ] {
        let server = MockServer::start().await;
        let final_result =
            json!({"resultType": "complete", "content": [{"type": "text", "text": "done"}]});
        let final_response = final_result.clone();
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().unwrap();
                match body["method"].as_str() {
                    Some("server/discover") => discover_response(&body),
                    Some("tools/call") if body.pointer("/params/inputResponses").is_none() => {
                        let mut result = input_required(native_request());
                        result["inputRequests"]["confirmation"] = elicitation_request("form");
                        result["inputRequests"]["richConfirmation"] = rich_form_request();
                        sse_result_response(&body, result)
                    }
                    Some("tools/call") => {
                        assert_eq!(
                            body["params"]["inputResponses"],
                            json!({
                                "verification": expected,
                                "confirmation": {"action": "accept", "content": {"confirmed": true}},
                                "richConfirmation": {"action": "accept", "content": {"confirmed": true}}
                            })
                        );
                        assert_eq!(body["params"]["requestState"], OPAQUE_STATE);
                        assert_eq!(body["params"]["name"], "verified_operation");
                        assert_eq!(body["params"]["arguments"], json!({"resource": "example"}));
                        assert_eq!(body["params"]["_meta"]["caller"], "preserved");
                        result_response(&body, final_response.clone())
                    }
                    other => panic!("unexpected request: {other:?}"),
                }
            })
            .expect(/*r*/ 3)
            .mount(&server)
            .await;
        let mut capabilities = capabilities();
        capabilities
            .extensions
            .as_mut()
            .unwrap()
            .get_mut(OPENAI_ELICITATION_EXTENSION_ID)
            .unwrap()
            .insert("form".into(), json!({}));
        capabilities.elicitation =
            Some(ElicitationCapability::new().with_form(FormElicitationCapability::new()));
        let client = client(
            &server,
            capabilities,
            Box::new(move |_, request| {
                if let Elicitation::OpenAiElicitationForm { meta, message, requested_schema } = request {
                    let expected = rich_form_request();
                    assert_eq!(
                        json!({"_meta": meta, "message": message, "requestedSchema": requested_schema}),
                        json!({
                            "_meta": expected["params"]["_meta"],
                            "message": expected["params"]["message"],
                            "requestedSchema": expected["params"]["requestedSchema"]
                        })
                    );
                    return Box::pin(async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Accept,
                            content: Some(json!({"confirmed": true})),
                            meta: None,
                        })
                    });
                }
                if let Elicitation::Mcp(ElicitRequestParams::FormElicitationParams {
                    meta, ..
                }) = request
                {
                    assert_eq!(
                        meta.and_then(|meta| meta.get("inputContext").cloned()),
                        Some(json!("server-context"))
                    );
                    return Box::pin(async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Accept,
                            content: Some(json!({"confirmed": true})),
                            meta: None,
                        })
                    });
                }
                assert_eq!(
                    request,
                    Elicitation::UserVerification {
                        meta: Some(json!({"example/display": {"label": "Operation"}})),
                        title: "Verify action".into(),
                        description: "Verify the requested operation".into(),
                        challenge: "AQID".into(),
                    }
                );
                let response = response(action.clone(), content.clone());
                Box::pin(async move { Ok(response) })
            }),
        )
        .await?;
        assert_eq!(call(&client).await?, final_result);
        let calls: Vec<_> = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .map(|request| request.body_json::<Value>().unwrap())
            .filter(|body| body["method"] == "tools/call")
            .collect();
        assert_ne!(calls[0]["id"], calls[1]["id"]);
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn native_mrtr_bounds_repeated_input_requests() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") => result_response(&body, input_required(native_request())),
                other => panic!("unexpected request: {other:?}"),
            }
        })
        .expect(1 + rmcp::model::DEFAULT_MRTR_MAX_ROUNDS as u64)
        .mount(&server)
        .await;
    let client = client(
        &server,
        capabilities(),
        Box::new(|_, _| {
            Box::pin(async {
                Ok(response(
                    ElicitationAction::Accept,
                    json!({"credentialId": "test", "signature": "BAUG"}),
                ))
            })
        }),
    )
    .await?;
    let error = call(&client).await.unwrap_err();
    assert!(error.to_string().contains("MRTR"), "{error}");
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn native_mrtr_rejects_unadvertised_and_malformed_requests_without_prompting()
-> anyhow::Result<()> {
    let mut malformed = native_request();
    malformed["params"]["challenge"] = json!("not base64!");
    for (capabilities, request) in [
        (ClientCapabilities::default(), native_request()),
        (capabilities(), rich_form_request()),
        (capabilities(), malformed),
        (
            capabilities(),
            json!({"method": "arbitrary/custom", "params": {}}),
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |http: &Request| {
                let body: Value = http.body_json().unwrap();
                match body["method"].as_str() {
                    Some("server/discover") => discover_response(&body),
                    Some("tools/call") => result_response(&body, input_required(request.clone())),
                    other => panic!("unexpected request: {other:?}"),
                }
            })
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        let client = client(
            &server,
            capabilities,
            Box::new(|_, _| panic!("must not prompt")),
        )
        .await?;
        assert!(call(&client).await.is_err());
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn mrtr_rejects_unsupported_elicitation_modes_without_prompting() -> anyhow::Result<()> {
    for mode in [None, Some(json!(null)), Some(json!(0)), Some(json!("url"))] {
        let server = MockServer::start().await;
        let mut input = native_request();
        if let Some(mode) = mode {
            input["params"]["mode"] = mode;
        } else {
            input["params"].as_object_mut().unwrap().remove("mode");
        }
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().unwrap();
                match body["method"].as_str() {
                    Some("server/discover") => discover_response(&body),
                    Some("tools/call") => result_response(&body, input_required(input.clone())),
                    other => panic!("unexpected request: {other:?}"),
                }
            })
            .expect(/*r*/ 2)
            .mount(&server)
            .await;
        let client = client(
            &server,
            capabilities(),
            Box::new(|_, _| panic!("must not prompt")),
        )
        .await?;
        let error = call(&client).await.unwrap_err();
        assert_eq!(
            codex_rmcp_client::mcp_error(&error),
            Some(&rmcp::ErrorData::invalid_request(
                "unsupported OpenAI elicitation mode",
                /*data*/ None
            ))
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn mrtr_preserves_auth_challenges_after_input_responses() -> anyhow::Result<()> {
    for (request, action, content) in [
        (
            elicitation_request("form"),
            ElicitationAction::Accept,
            json!({"confirmed": true}),
        ),
        (
            rich_form_request(),
            ElicitationAction::Accept,
            json!({"confirmed": true}),
        ),
        (
            native_request(),
            ElicitationAction::Accept,
            json!({"credentialId": "test", "signature": "BAUG"}),
        ),
        (native_request(), ElicitationAction::Cancel, Value::Null),
        (native_request(), ElicitationAction::Decline, Value::Null),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |http: &Request| {
                let body: Value = http.body_json().unwrap();
                match body["method"].as_str() {
                    Some("server/discover") => discover_response(&body),
                    Some("tools/call") if body.pointer("/params/inputResponses").is_none() => {
                        result_response(&body, input_required(request.clone()))
                    }
                    Some("tools/call") => ResponseTemplate::new(/*s*/ 401)
                        .insert_header("www-authenticate", "Bearer error=\"invalid_token\""),
                    other => panic!("unexpected request: {other:?}"),
                }
            })
            .expect(/*r*/ 3)
            .mount(&server)
            .await;
        let mut capabilities = capabilities();
        capabilities
            .extensions
            .as_mut()
            .unwrap()
            .get_mut(OPENAI_ELICITATION_EXTENSION_ID)
            .unwrap()
            .insert("form".into(), json!({}));
        capabilities.elicitation =
            Some(ElicitationCapability::new().with_form(FormElicitationCapability::new()));
        let client = client(
            &server,
            capabilities,
            Box::new(move |_, _| {
                let response = response(action.clone(), content.clone());
                Box::pin(async move { Ok(response) })
            }),
        )
        .await?;
        assert_eq!(
            call(&client).await?,
            serde_json::to_value(
                rmcp::model::CallToolResult::error(vec![rmcp::model::ContentBlock::text(
                    "Authentication required",
                )])
                .with_meta(Some(rmcp::model::MetaObject::from(
                    serde_json::Map::from_iter([(
                        "mcp/www_authenticate".into(),
                        json!(["Bearer error=\"invalid_token\""]),
                    )])
                )))
            )?
        );
        client.shutdown().await;
        server.verify().await;
    }
    Ok(())
}

#[tokio::test]
async fn native_mrtr_does_not_restart_after_continuation_session_expiry() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") if body.pointer("/params/inputResponses").is_none() => {
                    result_response(&body, input_required(native_request()))
                }
                Some("tools/call") => ResponseTemplate::new(/*s*/ 404),
                other => panic!("unexpected request: {other:?}"),
            }
        })
        .expect(/*r*/ 3)
        .mount(&server)
        .await;
    let client = client(
        &server,
        capabilities(),
        Box::new(|_, _| {
            Box::pin(async {
                Ok(response(
                    ElicitationAction::Accept,
                    json!({"credentialId": "test", "signature": "BAUG"}),
                ))
            })
        }),
    )
    .await?;
    let error = call(&client).await.unwrap_err();
    assert!(
        error.to_string().contains("the tool was not restarted"),
        "{error}"
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn native_mrtr_does_not_restart_after_proof_then_state_only_round() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") if body.pointer("/params/requestState").is_none() => {
                    result_response(&body, input_required(native_request()))
                }
                Some("tools/call") if body.pointer("/params/inputResponses").is_some() => {
                    result_response(
                        &body,
                        json!({"resultType": "input_required", "requestState": "after-proof"}),
                    )
                }
                Some("tools/call") => ResponseTemplate::new(/*s*/ 404),
                other => panic!("unexpected request: {other:?}"),
            }
        })
        .expect(/*r*/ 4)
        .mount(&server)
        .await;
    let client = client(
        &server,
        capabilities(),
        Box::new(|_, _| {
            Box::pin(async {
                Ok(response(
                    ElicitationAction::Accept,
                    json!({"credentialId": "test", "signature": "BAUG"}),
                ))
            })
        }),
    )
    .await?;
    let error = call(&client).await.unwrap_err();
    assert!(
        error.to_string().contains("the tool was not restarted"),
        "{error}"
    );
    client.shutdown().await;
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn native_mrtr_concurrent_prompts_have_independent_cancellation() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            match body["method"].as_str() {
                Some("server/discover") => discover_response(&body),
                Some("tools/call") if body.pointer("/params/inputResponses").is_none() => {
                    result_response(&body, input_required(native_request()))
                }
                Some("tools/call") => {
                    assert_eq!(
                        body["params"]["inputResponses"],
                        json!({"verification": {"action": "cancel"}})
                    );
                    result_response(&body, json!({"resultType": "complete", "content": []}))
                }
                other => panic!("unexpected request: {other:?}"),
            }
        })
        .expect(/*r*/ 4)
        .mount(&server)
        .await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let client = client(
        &server,
        capabilities(),
        Box::new(move |id, _| {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send((id, reply_tx)).unwrap();
            Box::pin(async move { Ok(reply_rx.await?) })
        }),
    )
    .await?;
    let first_client = Arc::clone(&client);
    let first_call = tokio::spawn(async move { call(&first_client).await });
    let (first_id, mut first_reply) = timeout(Duration::from_secs(/*secs*/ 5), rx.recv())
        .await?
        .unwrap();
    let second_client = Arc::clone(&client);
    let second_call = tokio::spawn(async move { call(&second_client).await });
    let (second_id, second_reply) = timeout(Duration::from_secs(/*secs*/ 5), rx.recv())
        .await?
        .unwrap();
    assert_ne!(first_id, second_id);
    first_call.abort();
    assert!(first_call.await.unwrap_err().is_cancelled());
    timeout(Duration::from_secs(/*secs*/ 5), first_reply.closed()).await?;
    assert!(!second_reply.is_closed());
    second_reply
        .send(response(ElicitationAction::Cancel, Value::Null))
        .unwrap();
    assert_eq!(
        timeout(Duration::from_secs(/*secs*/ 5), second_call).await???,
        json!({"resultType": "complete", "content": []})
    );
    client.shutdown().await;
    Ok(())
}

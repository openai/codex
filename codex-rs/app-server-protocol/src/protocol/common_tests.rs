use super::*;
use anyhow::Result;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn client_response_payload_serializes_without_an_intermediate_json_value() -> Result<()> {
    let payload = ClientResponsePayload::ThreadArchive(v2::ThreadArchiveResponse {});
    assert_eq!(serde_json::to_string(&payload)?, "{}");
    let Some(ClientResponse::ThreadArchive {
        request_id,
        response: _,
    }) = payload.into_client_response(RequestId::Integer(7))
    else {
        panic!("expected thread/archive client response");
    };
    assert_eq!(request_id, RequestId::Integer(7));
    Ok(())
}

#[test]
fn interrupt_conversation_payload_stays_jsonrpc_only() -> Result<()> {
    let payload = ClientResponsePayload::InterruptConversation(v1::InterruptConversationResponse {
        abort_reason: TurnAbortReason::Interrupted,
    });
    assert_eq!(
        serde_json::to_value(&payload)?,
        json!({
            "abortReason": "interrupted",
        })
    );
    assert!(
        payload
            .into_client_response(RequestId::Integer(8))
            .is_none()
    );
    Ok(())
}

#[test]
fn client_response_jsonrpc_parts_preserve_payloads_and_request_ids() -> Result<()> {
    let payloads = [
        (
            ClientResponsePayload::GetAuthStatus(v1::GetAuthStatusResponse {
                auth_method: Some(AuthMode::Chatgpt),
                auth_token: None,
                requires_openai_auth: Some(true),
            }),
            json!({
                "authMethod": "chatgpt",
                "authToken": null,
                "requiresOpenaiAuth": true,
            }),
        ),
        (
            ClientResponsePayload::GetAccount(v2::GetAccountResponse {
                account: Some(v2::Account::ApiKey {}),
                requires_openai_auth: false,
                workspace_routing: None,
            }),
            json!({
                "account": { "type": "apiKey" },
                "requiresOpenaiAuth": false,
                "workspaceRouting": null,
            }),
        ),
    ];

    for (payload, expected_result) in payloads {
        for request_id in [RequestId::Integer(7), RequestId::String("request-7".into())] {
            let expected = (request_id.clone(), expected_result.clone());
            let response = payload
                .clone()
                .into_client_response(request_id.clone())
                .expect("request-backed payload has a typed response");

            assert_eq!(response.into_jsonrpc_parts()?, expected);
            assert_eq!(payload.to_jsonrpc_parts(request_id.clone())?, expected);
            assert_eq!(payload.clone().into_jsonrpc_parts(request_id)?, expected);
        }
    }
    Ok(())
}

#[test]
fn client_response_jsonrpc_parts_preserve_legacy_interrupt() -> Result<()> {
    let request_id = RequestId::String("interrupt-7".into());
    let payload = ClientResponsePayload::InterruptConversation(v1::InterruptConversationResponse {
        abort_reason: TurnAbortReason::Interrupted,
    });
    let expected = (request_id.clone(), json!({ "abortReason": "interrupted" }));

    assert!(
        payload
            .clone()
            .into_client_response(request_id.clone())
            .is_none()
    );
    assert_eq!(payload.to_jsonrpc_parts(request_id.clone())?, expected);
    assert_eq!(payload.into_jsonrpc_parts(request_id)?, expected);
    Ok(())
}

#[cfg(unix)]
#[test]
fn client_response_jsonrpc_parts_preserve_serialization_errors() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let request_id = RequestId::Integer(7);
    let response = v1::GetConversationSummaryResponse {
        summary: v1::ConversationSummary {
            conversation_id: codex_protocol::ThreadId::from_u128(/*value*/ 7),
            path: PathBuf::from(OsString::from_vec(vec![0xff])),
            preview: String::new(),
            timestamp: None,
            updated_at: None,
            model_provider: String::new(),
            cwd: PathBuf::new(),
            cli_version: String::new(),
            source: codex_protocol::protocol::SessionSource::Exec,
            git_info: None,
        },
    };
    let describe = |error: serde_json::Error| (error.classify(), error.to_string());
    let expected = describe(serde_json::to_value(&response).unwrap_err());
    let payload = ClientResponsePayload::GetConversationSummary(response.clone());
    let typed = ClientResponse::GetConversationSummary {
        request_id: request_id.clone(),
        response,
    };

    assert_eq!(describe(typed.into_jsonrpc_parts().unwrap_err()), expected);
    assert_eq!(
        describe(payload.to_jsonrpc_parts(request_id.clone()).unwrap_err()),
        expected
    );
    assert_eq!(
        describe(payload.into_jsonrpc_parts(request_id).unwrap_err()),
        expected
    );
}

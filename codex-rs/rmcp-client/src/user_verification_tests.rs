use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn user_verification_request_preserves_signed_bytes_and_display_text() {
    assert_eq!(
        parse_request(CustomRequest::new(
            "openai/elicitation/create",
            Some(json!({
                "mode": MODE,
                "title": "Approve purchase",
                "description": "Pay $200 to Example Store",
                "challenge": "AQID",
            })),
        ))
        .unwrap(),
        Elicitation::UserVerification {
            meta: None,
            title: "Approve purchase".to_string(),
            description: "Pay $200 to Example Store".to_string(),
            challenge: "AQID".to_string(),
        }
    );
}

#[test]
fn user_verification_request_metadata_matches_mcp_elicitation() {
    for meta in [
        None,
        Some(json!(null)),
        Some(json!({})),
        Some(json!({"example/display": {"label": "Operation", "details": [1, true, null]}})),
        Some(json!(true)),
        Some(json!(7)),
        Some(json!("scalar")),
        Some(json!([])),
    ] {
        let mut params = json!({
            "mode": MODE,
            "title": "Approve",
            "description": "Review operation",
            "challenge": "AQID",
        });
        let mut mcp_params = json!({
            "mode": "form",
            "message": "Approve",
            "requestedSchema": {"type": "object", "properties": {}},
        });
        if let Some(meta) = &meta {
            params["_meta"] = meta.clone();
            mcp_params["_meta"] = meta.clone();
        }
        let mcp_request = serde_json::from_value::<rmcp::model::ElicitRequestParams>(mcp_params);
        let expected = if mcp_request.is_ok() {
            Ok(Elicitation::UserVerification {
                meta: meta.filter(serde_json::Value::is_object),
                title: "Approve".into(),
                description: "Review operation".into(),
                challenge: "AQID".into(),
            })
        } else {
            Err(invalid_request())
        };
        assert_eq!(
            parse_request(CustomRequest::new(
                "openai/elicitation/create",
                Some(params),
            )),
            expected,
        );
    }
}

#[test]
fn user_verification_rejects_invalid_or_unbounded_requests_without_echoing_values() {
    for (field, value) in [
        ("mode", json!("unknown")),
        ("title", json!("")),
        ("title", json!("x".repeat(257))),
        ("description", json!("x".repeat(4097))),
        ("challenge", json!("")),
        ("challenge", json!("not base64url!")),
        ("challenge", json!(URL_SAFE_NO_PAD.encode(vec![0; 4097]))),
        ("unexpected", json!("secret")),
    ] {
        let mut params = json!({
            "mode": MODE, "title": "Approve", "description": "", "challenge": "AQID"
        });
        params[field] = value;
        assert_eq!(
            parse_request(CustomRequest::new(
                "openai/elicitation/create",
                Some(params)
            )),
            Err(invalid_request()),
        );
    }
}

#[test]
fn user_verification_acceptance_returns_proof_only_in_content() {
    let content = Some(json!({"credentialId": "credential:用户/1=", "signature": "BAUG"}));
    assert_eq!(
        validate_response(ElicitationResponse {
            action: ElicitationAction::Accept,
            content: content.clone(),
            meta: Some(json!({"untrusted": "ignored"})),
        }),
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content,
            meta: None,
        }
    );
}

#[test]
fn user_verification_cancels_acceptance_without_a_valid_bounded_proof() {
    for content in [
        None,
        Some(json!({})),
        Some(json!({"credentialId": "AQID", "signature": ""})),
        Some(json!({"credentialId": "AQID", "signature": "BAUG="})),
        Some(json!({"credentialId": "é".repeat(513), "signature": "BAUG"})),
        Some(json!({"credentialId": "", "signature": "BAUG"})),
        Some(json!({"credentialId": "AQID", "signature": URL_SAFE_NO_PAD.encode(vec![0; 129])})),
        Some(json!({"credentialId": "AQID", "signature": "BAUG", "extra": true})),
    ] {
        assert_eq!(
            validate_response(ElicitationResponse {
                action: ElicitationAction::Accept,
                content,
                meta: None,
            }),
            ElicitationResponse {
                action: ElicitationAction::Cancel,
                content: None,
                meta: None,
            }
        );
    }
}

#[test]
fn user_verification_decline_and_cancel_discard_proof_material() {
    for action in [ElicitationAction::Decline, ElicitationAction::Cancel] {
        assert_eq!(
            validate_response(ElicitationResponse {
                action: action.clone(),
                content: Some(json!({"credentialId": "AQID", "signature": "BAUG"})),
                meta: Some(json!({"secret": "discarded"})),
            }),
            ElicitationResponse {
                action,
                content: None,
                meta: None
            }
        );
    }
}

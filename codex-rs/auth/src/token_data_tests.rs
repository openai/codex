use super::*;
use pretty_assertions::assert_eq;
use serde::Serialize;

#[test]
fn id_token_info_parses_email_and_plan() {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = serde_json::json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": "pro"
        }
    });

    fn b64url_no_pad(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
    let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
    let signature_b64 = b64url_no_pad(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let info = parse_chatgpt_jwt_claims(&fake_jwt).expect("should parse");
    assert_eq!(info.email.as_deref(), Some("user@example.com"));
    assert_eq!(info.get_chatgpt_plan_type().as_deref(), Some("Pro"));
}

#[test]
fn id_token_info_parses_go_plan() {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = serde_json::json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": "go"
        }
    });

    fn b64url_no_pad(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
    let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
    let signature_b64 = b64url_no_pad(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let info = parse_chatgpt_jwt_claims(&fake_jwt).expect("should parse");
    assert_eq!(info.email.as_deref(), Some("user@example.com"));
    assert_eq!(info.get_chatgpt_plan_type().as_deref(), Some("Go"));
}

#[test]
fn id_token_info_handles_missing_fields() {
    #[derive(Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = serde_json::json!({ "sub": "123" });

    fn b64url_no_pad(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
    let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
    let signature_b64 = b64url_no_pad(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

    let info = parse_chatgpt_jwt_claims(&fake_jwt).expect("should parse");
    assert!(info.email.is_none());
    assert!(info.get_chatgpt_plan_type().is_none());
}

#[test]
fn workspace_account_detection_matches_workspace_plans() {
    let workspace = IdTokenInfo {
        chatgpt_plan_type: Some(PlanType::Known(KnownPlan::Business)),
        ..IdTokenInfo::default()
    };
    assert_eq!(workspace.is_workspace_account(), true);

    let personal = IdTokenInfo {
        chatgpt_plan_type: Some(PlanType::Known(KnownPlan::Pro)),
        ..IdTokenInfo::default()
    };
    assert_eq!(personal.is_workspace_account(), false);
}

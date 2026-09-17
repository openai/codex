//! Local account-user identity selection must use the access token's selected workspace.

use super::*;
use base64::Engine;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

fn access_token(auth_claims: Value) -> String {
    let payload = json!({"https://api.openai.com/auth": auth_claims});
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).unwrap());
    format!("e30.{payload}.c2ln")
}

fn managed_auth(access_token: String, selected_account: Option<&str>) -> CodexAuth {
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let CodexAuth::Chatgpt(managed) = &auth else {
        panic!("expected managed ChatGPT auth");
    };
    let mut state = managed.state.auth_dot_json.lock().unwrap();
    let tokens = state.as_mut().unwrap().tokens.as_mut().unwrap();
    tokens.access_token = access_token;
    tokens.account_id = selected_account.map(str::to_owned);
    tokens.id_token.chatgpt_user_id = Some("different-id-token-user".to_string());
    drop(state);
    auth
}

#[test]
fn account_user_id_comes_from_the_access_token_without_synthesis() {
    let auth = managed_auth(
        access_token(json!({
            "chatgpt_account_user_id": "opaque-membership",
            "chatgpt_account_id": "workspace-a",
            "chatgpt_user_id": "chatgpt-user",
            "user_id": "auth-user",
        })),
        Some("workspace-a"),
    );

    assert_eq!(
        auth.get_chatgpt_account_user_id(),
        Some("opaque-membership".to_string())
    );
}

#[test]
fn account_user_id_rejects_missing_malformed_or_unscoped_claims() {
    for claims in [
        json!(null),
        json!({"user_id": "auth-user", "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_user_id": "chatgpt-user", "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_account_user_id": null, "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_account_user_id": "", "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_account_user_id": " membership ", "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_account_user_id": 123, "chatgpt_account_id": "workspace-a"}),
        json!({"chatgpt_account_user_id": "membership"}),
        json!({"chatgpt_account_user_id": "membership", "chatgpt_account_id": "workspace-b"}),
        json!({"chatgpt_account_user_id": "membership", "chatgpt_account_id": 123}),
    ] {
        let auth = managed_auth(access_token(claims.clone()), Some("workspace-a"));
        assert_eq!(auth.get_chatgpt_account_user_id(), None, "{claims}");
    }

    for token in [
        "not-a-jwt".to_string(),
        "e30.!.c2ln".to_string(),
        format!(
            "{}.extra",
            access_token(json!({
                "chatgpt_account_user_id": "membership",
                "chatgpt_account_id": "workspace-a",
            }))
        ),
    ] {
        let auth = managed_auth(token, Some("workspace-a"));
        assert_eq!(auth.get_chatgpt_account_user_id(), None);
    }
}

#[test]
fn account_user_id_requires_a_selected_account() {
    for selected_account in [None, Some(""), Some(" workspace-a ")] {
        let auth = managed_auth(
            access_token(json!({
                "chatgpt_account_user_id": "membership",
                "chatgpt_account_id": "workspace-a",
            })),
            selected_account,
        );
        assert_eq!(auth.get_chatgpt_account_user_id(), None);
    }
}

#[test]
fn external_account_user_id_must_match_the_selected_workspace() {
    let token = access_token(json!({
        "chatgpt_account_user_id": "membership-a",
        "chatgpt_account_id": "workspace-a",
    }));
    for (selected_account, expected) in [
        ("workspace-a", Some("membership-a".to_string())),
        ("workspace-b", None),
    ] {
        let auth = CodexAuth::from_external_chatgpt_tokens(
            &token,
            selected_account,
            /*chatgpt_plan_type*/ None,
        )
        .unwrap();
        assert_eq!(auth.get_chatgpt_account_user_id(), expected);
    }
}

#[test]
fn malformed_account_user_id_does_not_break_ordinary_external_auth() {
    let token = access_token(json!({
        "chatgpt_account_user_id": 123,
        "chatgpt_account_id": "workspace-a",
        "chatgpt_user_id": "chatgpt-user",
    }));
    let auth = CodexAuth::from_external_chatgpt_tokens(
        &token,
        "workspace-a",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();

    assert_eq!(
        (
            auth.get_chatgpt_account_user_id(),
            auth.get_chatgpt_user_id(),
            auth.get_account_id(),
        ),
        (
            None,
            Some("chatgpt-user".to_string()),
            Some("workspace-a".to_string()),
        )
    );
}

#[test]
fn api_key_and_header_auth_do_not_expose_an_account_user_id() {
    let mut headers = http::HeaderMap::new();
    headers.insert(
        "chatgpt-account-id",
        http::HeaderValue::from_static("workspace-a"),
    );
    for auth in [
        CodexAuth::from_api_key("test-api-key"),
        CodexAuth::Headers(AuthHeaders::new(headers)),
    ] {
        assert_eq!(auth.get_chatgpt_account_user_id(), None);
    }
}

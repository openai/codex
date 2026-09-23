use super::*;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::Hmac;
use hmac::Mac;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;

type HmacSha256 = Hmac<Sha256>;

fn signed_token(shared_secret: &[u8], claims: serde_json::Value) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims_segment = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
    let payload = format!("{header}.{claims_segment}");
    let mut mac = HmacSha256::new_from_slice(shared_secret).unwrap();
    mac.update(payload.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{payload}.{signature}")
}

#[test]
fn detects_unauthenticated_non_loopback_listener() {
    let policy = WebsocketAuthPolicy::default();
    assert!(is_unauthenticated_non_loopback_listener(
        "0.0.0.0:8765".parse().unwrap(),
        &policy,
    ));
    assert!(!is_unauthenticated_non_loopback_listener(
        "127.0.0.1:8765".parse().unwrap(),
        &policy,
    ));
    assert!(!is_unauthenticated_non_loopback_listener(
        "0.0.0.0:8765".parse().unwrap(),
        &WebsocketAuthPolicy {
            mode: Some(WebsocketAuthMode::CapabilityToken {
                token_sha256: [0u8; 32],
            }),
        },
    ));
}

#[test]
fn capability_token_args_require_token_file_or_hash() {
    let err = WebsocketAuthArgs {
        ws_auth: Some(WebsocketAuthCliMode::CapabilityToken),
        ..Default::default()
    }
    .try_into_settings()
    .expect_err("capability-token mode should require a token source");
    assert!(
        err.to_string().contains("--ws-token-file")
            && err.to_string().contains("--ws-token-sha256"),
        "unexpected error: {err}"
    );
}

#[test]
fn capability_token_args_accept_token_hash() {
    let settings = WebsocketAuthArgs {
        ws_auth: Some(WebsocketAuthCliMode::CapabilityToken),
        ws_token_sha256: Some("ab".repeat(32)),
        ..Default::default()
    }
    .try_into_settings()
    .expect("capability-token hash args should parse");

    assert_eq!(
        settings,
        WebsocketAuthSettings {
            config: Some(WebsocketAuthConfig::CapabilityToken {
                source: WebsocketCapabilityTokenSource::TokenSha256 {
                    token_sha256: [0xab; 32],
                },
            }),
        }
    );
}

#[test]
fn capability_token_args_reject_multiple_token_sources() {
    let err = WebsocketAuthArgs {
        ws_auth: Some(WebsocketAuthCliMode::CapabilityToken),
        ws_token_file: Some(PathBuf::from("/tmp/token")),
        ws_token_sha256: Some("ab".repeat(32)),
        ..Default::default()
    }
    .try_into_settings()
    .expect_err("capability-token mode should reject multiple token sources");
    assert!(
        err.to_string().contains("mutually exclusive"),
        "unexpected error: {err}"
    );
}

#[test]
fn capability_token_args_reject_malformed_token_hash() {
    let err = WebsocketAuthArgs {
        ws_auth: Some(WebsocketAuthCliMode::CapabilityToken),
        ws_token_sha256: Some("not-a-sha256".to_string()),
        ..Default::default()
    }
    .try_into_settings()
    .expect_err("capability-token mode should reject malformed token hashes");
    assert!(
        err.to_string().contains("64-character hex"),
        "unexpected error: {err}"
    );
}

#[test]
fn capability_token_hash_policy_authorizes_matching_bearer_token() {
    let settings = WebsocketAuthSettings {
        config: Some(WebsocketAuthConfig::CapabilityToken {
            source: WebsocketCapabilityTokenSource::TokenSha256 {
                token_sha256: sha256_digest(b"super-secret-token"),
            },
        }),
    };
    let policy = policy_from_settings(&settings).expect("hash policy should build");
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer super-secret-token"),
    );
    authorize_upgrade(&headers, &policy).expect("matching token should authorize");

    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer wrong-token"),
    );
    let err = authorize_upgrade(&headers, &policy).expect_err("wrong token should fail");
    assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
}

#[test]
fn signed_bearer_args_require_mode_when_mode_specific_flags_are_set() {
    let err = WebsocketAuthArgs {
        ws_shared_secret_file: Some(PathBuf::from("/tmp/secret")),
        ..Default::default()
    }
    .try_into_settings()
    .expect_err("mode-specific flags should require --ws-auth");
    assert!(
        err.to_string().contains("websocket auth flags require"),
        "unexpected error: {err}"
    );
}

#[test]
fn signed_bearer_args_default_clock_skew_and_trim_optional_claims() {
    let settings = WebsocketAuthArgs {
        ws_auth: Some(WebsocketAuthCliMode::SignedBearerToken),
        ws_shared_secret_file: Some(PathBuf::from("/tmp/secret")),
        ws_issuer: Some(" issuer ".to_string()),
        ws_audience: Some("   ".to_string()),
        ..Default::default()
    }
    .try_into_settings()
    .expect("signed bearer args should parse");

    assert_eq!(
        settings,
        WebsocketAuthSettings {
            config: Some(WebsocketAuthConfig::SignedBearerToken {
                shared_secret_file: AbsolutePathBuf::from_absolute_path("/tmp/secret")
                    .expect("absolute path"),
                issuer: Some("issuer".to_string()),
                audience: None,
                max_clock_skew_seconds: DEFAULT_MAX_CLOCK_SKEW_SECONDS,
            }),
        }
    );
}

#[test]
fn signed_bearer_token_verification_rejects_tampering() {
    let shared_secret = b"0123456789abcdef0123456789abcdef";
    let token = signed_token(
        shared_secret,
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
        }),
    );
    let tampered = token.replace(".eyJleHAi", ".eyJleHBi");
    let err = verify_signed_bearer_token(
        &tampered,
        shared_secret,
        /*issuer*/ None,
        /*audience*/ None,
        /*max_clock_skew_seconds*/ 30,
    )
    .expect_err("tampered jwt should fail");
    assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
}

#[test]
fn signed_bearer_token_verification_accepts_valid_token() {
    let shared_secret = b"0123456789abcdef0123456789abcdef";
    let token = signed_token(
        shared_secret,
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "iss": "issuer",
            "aud": "audience",
        }),
    );
    verify_signed_bearer_token(
        &token,
        shared_secret,
        Some("issuer"),
        Some("audience"),
        /*max_clock_skew_seconds*/ 30,
    )
    .expect("valid signed token should verify");
}

#[test]
fn signed_bearer_token_verification_accepts_multiple_audiences() {
    let shared_secret = b"0123456789abcdef0123456789abcdef";
    let token = signed_token(
        shared_secret,
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "aud": ["other-audience", "audience"],
        }),
    );
    verify_signed_bearer_token(
        &token,
        shared_secret,
        /*issuer*/ None,
        Some("audience"),
        /*max_clock_skew_seconds*/ 30,
    )
    .expect("jwt audience arrays should verify");
}

#[test]
fn signed_bearer_token_verification_rejects_alg_none_tokens() {
    let claims_segment = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
        }))
        .unwrap(),
    );
    let header_segment = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let token = format!("{header_segment}.{claims_segment}.");
    let err = verify_signed_bearer_token(
        &token,
        b"0123456789abcdef0123456789abcdef",
        /*issuer*/ None,
        /*audience*/ None,
        /*max_clock_skew_seconds*/ 30,
    )
    .expect_err("alg=none jwt should be rejected");
    assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
}

#[test]
fn signed_bearer_token_verification_rejects_missing_exp() {
    let shared_secret = b"0123456789abcdef0123456789abcdef";
    let token = signed_token(
        shared_secret,
        json!({
            "iss": "issuer",
        }),
    );
    let err = verify_signed_bearer_token(
        &token,
        shared_secret,
        /*issuer*/ None,
        /*audience*/ None,
        /*max_clock_skew_seconds*/ 30,
    )
    .expect_err("jwt without exp should be rejected");
    assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
}

#[test]
fn validate_signed_bearer_secret_rejects_short_secret() {
    let err = validate_signed_bearer_secret(Path::new("/tmp/secret"), b"too-short")
        .expect_err("short shared secret should be rejected");
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("must be at least 32 bytes"),
        "unexpected error: {err}"
    );
}

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::ema_claims::ID_JAG_TOKEN_TYPE;
use crate::ema_claims::validate_oidc_identity_assertion;

fn http_client() -> Arc<dyn HttpClient> {
    Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    )))
}

fn unique_form_fields(body: &[u8]) -> HashMap<String, String> {
    let pairs = url::form_urlencoded::parse(body)
        .into_owned()
        .collect::<Vec<_>>();
    let fields = pairs.iter().cloned().collect::<HashMap<_, _>>();
    assert_eq!(
        pairs.len(),
        fields.len(),
        "OAuth form must not contain duplicate fields"
    );
    fields
}

fn jwt(claims: &Value) -> String {
    format!(
        "{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256","typ":"oauth-id-jag+jwt"}"#),
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("serialize claims"))
    )
}

fn claims(issuer: &str, audience: &str, resource: &str) -> Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time")
        .as_secs();
    json!({"iss":issuer,"aud":audience,"sub":"user","client_id":"mcp-client",
        "jti":"unique-jag","iat":now,"exp":now + 3600,"resource":resource,"scope":"files.read"})
}

fn jag_response(claims: &Value) -> Value {
    let mut response = json!({"access_token":jwt(claims),"issued_token_type":ID_JAG_TOKEN_TYPE,
        "token_type":"N_A","resource":claims["resource"]});
    if let Some(scope) = claims.get("scope") {
        response["scope"] = scope.clone();
    }
    response
}

fn token_response() -> Value {
    json!({"access_token":"resource-token","token_type":"Bearer","expires_in":300})
}

async fn exchange(server: &MockServer, refresh_token: &str) -> Result<EmaAccessToken> {
    let issuer = format!("{}/idp", server.uri());
    let audience = format!("{}/as", server.uri());
    exchange_id_jag(EmaIdJagExchangeRequest {
        resource: &format!("{}/mcp", server.uri()),
        scopes: &[],
        mcp_client_id: "mcp-client",
        authorization_server_issuer: &audience,
        authorization_server_token_endpoint: &format!("{audience}/token"),
        idp_token_endpoint: &format!("{issuer}/token"),
        idp_issuer: &issuer,
        idp_client_id: "idp-client",
        refresh_token: refresh_token.to_string(),
        idp_http_client: http_client(),
        resource_http_client: http_client(),
    })
    .await
}

#[tokio::test]
async fn public_client_round_trip_preserves_signed_narrowing() -> Result<()> {
    let requested_scopes = ["files.read".to_string(), "files.write".to_string()];
    let client = http_client();
    for (scopes, echo_scope, refresh_token) in [
        (requested_scopes.as_slice(), true, "opaque-refresh-token"),
        (&[], true, "opaque-refresh-token"),
        (&[], false, "opaque-refresh-token"),
        (requested_scopes.as_slice(), true, ""),
        (requested_scopes.as_slice(), true, " \t"),
    ] {
        let server = MockServer::start().await;
        let issuer = format!("{}/idp", server.uri());
        let audience = format!("{}/as", server.uri());
        let resource = format!("{}/mcp", server.uri());
        let mut jag = jag_response(&claims(&issuer, &audience, &resource));
        if !echo_scope {
            jag.as_object_mut()
                .expect("ID-JAG response")
                .remove("scope");
        }
        let valid = !refresh_token.trim().is_empty();
        for (endpoint, response) in [("/idp/token", jag.clone()), ("/as/token", token_response())] {
            Mock::given(method("POST"))
                .and(path(endpoint))
                .respond_with(ResponseTemplate::new(200).set_body_json(response))
                .expect(u64::from(valid))
                .mount(&server)
                .await;
        }
        let expected_subject = refresh_token.to_string();
        let result = exchange_id_jag(EmaIdJagExchangeRequest {
            resource: &resource,
            scopes,
            mcp_client_id: "mcp-client",
            authorization_server_issuer: &audience,
            authorization_server_token_endpoint: &format!("{audience}/token"),
            idp_token_endpoint: &format!("{issuer}/token"),
            idp_issuer: &issuer,
            idp_client_id: "idp-client",
            refresh_token: refresh_token.to_string(),
            idp_http_client: Arc::clone(&client),
            resource_http_client: Arc::clone(&client),
        })
        .boxed()
        .await;
        if !valid {
            assert!(result.is_err(), "invalid subject must fail before HTTP");
            assert!(
                server
                    .received_requests()
                    .await
                    .expect("requests")
                    .is_empty()
            );
            continue;
        }
        assert_eq!(
            result?,
            EmaAccessToken {
                access_token: "resource-token".to_string(),
                expires_in: Some(Duration::from_secs(300)),
            }
        );
        let requests = server.received_requests().await.expect("requests");
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .iter()
                .all(|request| request.headers.get("authorization").is_none())
        );
        let mut forms = requests
            .iter()
            .map(|request| unique_form_fields(&request.body))
            .collect::<Vec<_>>();
        assert_eq!(
            forms[0].remove("scope"),
            (!scopes.is_empty()).then(|| scopes.join(" "))
        );
        assert_eq!(
            forms[0],
            HashMap::from([
                (
                    "grant_type".to_string(),
                    TOKEN_EXCHANGE_GRANT_TYPE.to_string()
                ),
                (
                    "requested_token_type".to_string(),
                    ID_JAG_TOKEN_TYPE.to_string(),
                ),
                ("subject_token".to_string(), expected_subject),
                (
                    "subject_token_type".to_string(),
                    "urn:ietf:params:oauth:token-type:refresh_token".to_string(),
                ),
                ("audience".to_string(), audience.clone()),
                ("resource".to_string(), resource.clone()),
                ("client_id".to_string(), "idp-client".to_string()),
            ])
        );
        assert_eq!(
            forms[1],
            HashMap::from([
                ("grant_type".to_string(), JWT_BEARER_GRANT_TYPE.to_string()),
                (
                    "assertion".to_string(),
                    jag["access_token"].as_str().expect("JAG").to_string()
                ),
                ("client_id".to_string(), "mcp-client".to_string()),
            ])
        );
    }
    Ok(())
}

#[tokio::test]
async fn sdk_validation_rejects_unsupported_authorization_before_returning_a_bearer() {
    for location in [
        "signed claims",
        "IdP response",
        "resource response",
        "signed scope",
    ] {
        let server = MockServer::builder().start().await;
        let original = claims(
            &format!("{}/idp", server.uri()),
            &format!("{}/as", server.uri()),
            &format!("{}/mcp", server.uri()),
        );
        let mut jag = jag_response(&original);
        let mut bearer = token_response();
        let details = json!([{"type": "payment_initiation"}]);
        match location {
            "signed claims" | "signed scope" => {
                let mut changed = original;
                if location == "signed claims" {
                    changed["authorization_details"] = details;
                } else {
                    changed["scope"] = json!("files.read\tfiles.write");
                }
                jag = jag_response(&changed);
            }
            "IdP response" => jag["authorization_details"] = details,
            "resource response" => bearer["authorization_details"] = details,
            _ => unreachable!(),
        }
        let resource_requested = location == "resource response";
        for (endpoint, response, count) in [
            ("/idp/token", jag, 1),
            ("/as/token", bearer, u64::from(resource_requested)),
        ] {
            Mock::given(method("POST"))
                .and(path(endpoint))
                .respond_with(ResponseTemplate::new(200).set_body_json(response))
                .expect(count)
                .mount(&server)
                .await;
        }
        let error = exchange(&server, "refresh-token")
            .await
            .expect_err("SDK must reject unsupported authorization");
        assert_eq!(
            error.downcast_ref::<EmaError>(),
            Some(&EmaError::InvalidResponse {
                stage: if resource_requested {
                    EmaExchangeStage::ResourceAuthorizationServer
                } else {
                    EmaExchangeStage::IdentityProvider
                },
                message: if location == "signed scope" {
                    "malformed or duplicate scopes"
                } else {
                    "authorization_details is not supported"
                },
            }),
            "{location}"
        );
        assert_eq!(
            server.received_requests().await.expect("requests").len(),
            if resource_requested { 2 } else { 1 }
        );
    }
}

#[test]
fn identity_and_credential_destinations_are_bound() {
    for endpoint in [
        "http://idp.example/token",
        "https://user:pass@idp.example/token",
        "https://idp.example/token#fragment",
    ] {
        assert!(
            validate_ema_oauth_endpoint(endpoint, "IdP").is_err(),
            "accepted {endpoint}"
        );
    }
    let original = claims("https://idp.example", "idp-client", "https://mcp.example");
    assert!(
        validate_oidc_identity_assertion(&jwt(&original), "https://idp.example", "idp-client")
            .is_ok()
    );
    for (field, value) in [
        ("iss", json!("https://other.example")),
        ("aud", json!(["idp-client", "other"])),
        ("azp", json!("other")),
        ("exp", json!(0)),
        ("sub", json!("")),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        assert!(
            validate_oidc_identity_assertion(&jwt(&changed), "https://idp.example", "idp-client")
                .is_err(),
            "accepted changed {field}"
        );
    }
}

#[tokio::test]
async fn provider_errors_preserve_credential_policy_without_reflecting_credentials() {
    const SENTINEL: &str = "secret-assertion-sentinel";
    for (endpoint, grant_source) in [
        ("/idp/token", EmaInvalidGrantSource::EnterpriseIdentity),
        ("/as/token", EmaInvalidGrantSource::ResourceAuthorization),
    ] {
        for code in [
            SENTINEL,
            "invalid_grant",
            "insufficient_user_authentication",
            "malformed",
        ] {
            let server = MockServer::builder().start().await;
            if endpoint == "/as/token" {
                let jag = jag_response(&claims(
                    &format!("{}/idp", server.uri()),
                    &format!("{}/as", server.uri()),
                    &format!("{}/mcp", server.uri()),
                ));
                Mock::given(method("POST"))
                    .and(path("/idp/token"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(jag))
                    .expect(1)
                    .mount(&server)
                    .await;
            }
            let response = if code == "malformed" {
                ResponseTemplate::new(200).set_body_json(json!({"expires_in": SENTINEL}))
            } else {
                ResponseTemplate::new(400).set_body_json(json!({
                    "error":code,"error_description":SENTINEL,
                }))
            };
            Mock::given(method("POST"))
                .and(path(endpoint))
                .respond_with(response)
                .expect(1)
                .mount(&server)
                .await;
            let error = exchange(&server, SENTINEL)
                .await
                .expect_err("provider error should fail");
            let expected = match code {
                "invalid_grant" => Some(EmaAuthFailure::InvalidGrant { grant_source }),
                "insufficient_user_authentication" => {
                    Some(EmaAuthFailure::InsufficientUserAuthentication)
                }
                _ => None,
            };
            assert_eq!(error.downcast_ref::<EmaAuthFailure>(), expected.as_ref());
            assert!(
                !format!("{error:#?}").contains(SENTINEL),
                "provider reflected a credential"
            );
            assert_eq!(
                server.received_requests().await.expect("requests").len(),
                if endpoint == "/as/token" { 2 } else { 1 },
                "exchange must not retry a rejected grant"
            );
        }
    }
}

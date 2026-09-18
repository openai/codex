use super::*;
use pretty_assertions::assert_eq;

fn provider(extra: &str) -> ModelProviderInfo {
    toml::from_str(&format!(
        r#"
base_url = "https://gateway.example.test/v1"
[gateway_oauth]
authorization_url = "https://login.example.test/authorize"
token_url = "https://login.example.test/token"
client_id = "codex"
{extra}
"#
    ))
    .unwrap()
}

#[test]
fn parses_header_and_cookie_delivery() {
    let header = provider("delivery = { kind = 'header', name = 'x-gateway-auth' }");
    assert_eq!(header.validate(), Ok(()));
    assert_eq!(
        header.gateway_oauth.as_ref().unwrap().delivery,
        GatewayOAuthDelivery::Header {
            name: "x-gateway-auth".into(),
            scheme: "Bearer".into(),
        }
    );
    let cookie = provider("delivery = { kind = 'cookie', name = 'gateway_session' }");
    assert_eq!(cookie.validate(), Ok(()));
    assert_eq!(
        toml::from_str::<ModelProviderInfo>(&toml::to_string(&cookie).unwrap()).unwrap(),
        cookie
    );
}

#[test]
fn rejects_unsafe_endpoints_and_delivery_collisions() {
    let valid = provider("delivery = { kind = 'header', name = 'x-gateway-auth' }");
    for url in [
        "http://remote.example.test",
        "https://user:secret@gateway.example.test",
        "https://gateway.example.test/#secret",
        "file:///token",
    ] {
        let mut invalid = valid.clone();
        invalid.base_url = Some(url.into());
        assert!(invalid.validate().is_err());
        invalid = valid.clone();
        invalid.gateway_oauth.as_mut().unwrap().token_url = url.into();
        let error = invalid.validate().unwrap_err();
        assert!(!error.contains(url));
    }
    for name in [
        "Authorization",
        "HOST",
        "x-openai-actor-authorization",
        "content-length",
        "sec-websocket-key",
        "bad name",
    ] {
        let invalid = provider(&format!(
            "delivery = {{ kind = 'header', name = '{name}' }}"
        ));
        assert!(invalid.validate().is_err(), "{name}");
    }
    let mut collision = valid;
    collision.http_headers = Some(std::collections::HashMap::from([(
        "X-Gateway-Auth".into(),
        "static".into(),
    )]));
    assert!(collision.validate().is_err());
    let mut cookie = provider("delivery = { kind = 'cookie', name = 'gateway' }");
    cookie.env_http_headers = Some(std::collections::HashMap::from([(
        "COOKIE".into(),
        "COOKIE_ENV".into(),
    )]));
    assert!(cookie.validate().is_err());
}

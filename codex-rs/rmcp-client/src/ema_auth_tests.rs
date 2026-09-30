//! Bound metadata selection and HTTP confinement run before enterprise credential access.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

fn request<'a>(resource: &'a str, issuer: Option<&'a str>) -> EmaAuthTokenExchangeRequest<'a> {
    let client: Arc<dyn HttpClient> = Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    )));
    EmaAuthTokenExchangeRequest {
        server_url: resource,
        resource: Some(resource),
        scopes: &[],
        mcp_client_id: Some("resource-client"),
        expected_authorization_server_issuer: issuer,
        idp_issuer: "https://idp.example",
        idp_client_id: "enterprise-client",
        idp_identity: Box::pin(async { bail!("identity resolver reached") }),
        idp_http_client: Arc::clone(&client),
        resource_http_client: client,
    }
}

#[tokio::test]
async fn discovery_pins_resource_issuer_and_public_jwt_bearer_before_identity() -> Result<()> {
    let metadata_cases = [
        (true, true, json!({}), true),
        (false, true, json!({}), false),
        (true, false, json!({}), false),
    ]
    .into_iter()
    .chain(
        [
            json!({"issuer": null}),
            json!({"issuer": "https://other.example"}),
            json!({"token_endpoint": "https://other.example/token"}),
            json!({"token_endpoint": "http://as.example/token"}),
            json!({"grant_types_supported": null}),
            json!({"grant_types_supported": ["authorization_code"]}),
            json!({"authorization_grant_profiles_supported": ["other"]}),
            json!({"token_endpoint_auth_methods_supported": ["private_key_jwt"]}),
        ]
        .into_iter()
        .map(|invalid| (true, true, invalid, false)),
    );
    let fallback_paths = [
        "/.well-known/oauth-protected-resource/enterprise",
        "/enterprise/.well-known/oauth-protected-resource",
        "/.well-known/oauth-protected-resource",
    ];
    let fallback_cases = fallback_paths.into_iter().flat_map(|path| {
        [true, false].map(move |valid| ("GET", path, valid, true, json!({}), valid))
    });
    for (metadata_method, metadata_path, resource_matches, issuer_matches, additions, valid) in
        metadata_cases
            .map(|(resource, issuer, additions, valid)| {
                (
                    "POST",
                    "/enterprise/tools",
                    resource,
                    issuer,
                    additions,
                    valid,
                )
            })
            .chain(fallback_cases)
    {
        let server = MockServer::start().await;
        let query = "tenant=one%2Ftwo&tenant=three+four&flag=&encoded=%2f";
        let resource = format!("{}/enterprise?{query}", server.uri());
        let transport = format!("{}/enterprise/tools?{query}", server.uri());
        let issuer = format!("{}/as", server.uri());
        // A query-only resource change must not be accepted as the same resource.
        let advertised_resource = if resource_matches {
            resource.clone()
        } else {
            resource.replace("one", "two")
        };
        Mock::given(method(metadata_method))
            .and(path(metadata_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": advertised_resource,
                "authorization_servers": [format!("{}/unused", server.uri()), issuer],
            })))
            .mount(&server)
            .await;
        let mut metadata = json!({"issuer": issuer, "token_endpoint": format!("{issuer}/token"),
            "grant_types_supported": [JWT_BEARER_GRANT_TYPE],
            "token_endpoint_auth_methods_supported": ["none"]});
        metadata
            .as_object_mut()
            .expect("metadata")
            .extend(additions.as_object().expect("overrides").clone());
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/as"))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
            .mount(&server)
            .await;
        let polled = AtomicBool::new(false);
        let mut exchange = request(
            &resource,
            Some(if issuer_matches {
                &issuer
            } else {
                "https://other.example"
            }),
        );
        exchange.server_url = &transport;
        exchange.idp_identity = Box::pin(async {
            polled.store(true, Ordering::SeqCst);
            bail!("identity resolver reached")
        });
        exchange_ema_auth_token(exchange)
            .await
            .expect_err("stop before token exchange");
        assert_eq!(polled.load(Ordering::SeqCst), valid, "{additions}");
        let requests = server.received_requests().await.expect("requests");
        let resource_requests = requests
            .iter()
            .filter(|request| request.url.path() != "/.well-known/oauth-authorization-server/as")
            .collect::<Vec<_>>();
        assert!(
            resource_requests
                .iter()
                .all(|request| request.url.query() == Some(query))
        );
        assert!(
            resource_requests
                .iter()
                .all(|request| request.url.path() == "/enterprise/tools"
                    || (metadata_method == "GET" && fallback_paths.contains(&request.url.path())))
        );
        assert_eq!(
            resource_requests
                .last()
                .expect("resource metadata request")
                .url
                .path(),
            metadata_path
        );
    }
    Ok(())
}

#[tokio::test]
async fn modern_challenge_confines_metadata_redirects() -> Result<()> {
    let server = MockServer::start().await;
    let authorization = MockServer::start().await;
    let attacker = MockServer::start().await;
    let resource = format!("{}/mcp", server.uri());
    let issuer = authorization.uri();
    let prm = format!("{}/enterprise/resource", server.uri());
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &wiremock::Request| {
            assert_eq!(request.headers["mcp-protocol-version"], "2026-07-28");
            assert_eq!(
                request.body_json::<Value>().expect("discovery")["method"],
                "server/discover"
            );
            ResponseTemplate::new(401).insert_header(
                "www-authenticate",
                format!("Bearer resource_metadata=\"{prm}\""),
            )
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/enterprise/resource"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"resource": resource, "authorization_servers": [issuer]})),
        )
        .mount(&server)
        .await;
    let metadata = json!({"issuer": issuer, "token_endpoint": format!("{issuer}/token"),
        "grant_types_supported": [JWT_BEARER_GRANT_TYPE],
        "token_endpoint_auth_methods_supported": ["none"]});
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(move |request: &wiremock::Request| {
            assert!(!request.headers.contains_key("authorization"));
            ResponseTemplate::new(200).set_body_json(metadata.clone())
        })
        .mount(&authorization)
        .await;
    let exchange = request(&resource, Some(&issuer));
    let (metadata, actual_resource) =
        discover_ema_authorization_metadata(EmaDiscoveryRequest::from(&exchange)).await?;
    assert_eq!(
        (metadata.issuer, metadata.token_endpoint, actual_resource),
        (
            Some(issuer.clone()),
            format!("{issuer}/token"),
            resource.clone()
        )
    );
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/capture", attacker.uri())),
        )
        .with_priority(1)
        .mount(&authorization)
        .await;
    let error = discover_ema_authorization_metadata(EmaDiscoveryRequest::from(&exchange))
        .await
        .err()
        .context("cross-origin redirect must fail")?;
    assert!(error.to_string().contains("cross-origin redirect"));
    assert!(
        attacker
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn legacy_challenges_require_a_recognized_discovery_rejection() -> Result<()> {
    for (status, id, code, message, legacy) in [
        (404, json!(null), -1, "", true),
        (405, json!(null), -1, "", true),
        (
            200,
            json!("ema-auth-discover"),
            -32601,
            "Method not found",
            true,
        ),
        (
            400,
            json!("ema-auth-discover"),
            -32022,
            "Unsupported protocol version: 2026-07-28",
            true,
        ),
        (
            400,
            json!(null),
            -32000,
            "Bad Request: No valid session ID provided",
            true,
        ),
        (200, json!("unrelated"), -32601, "Method not found", false),
        (400, json!(null), -32000, "Unrecognized rejection", false),
        (503, json!(null), -1, "", false),
    ] {
        for (sse, get_status) in [
            (false, 401),
            (false, 400),
            (false, 404),
            (false, 405),
            (false, 406),
            (true, 401),
            (true, 405),
        ] {
            let initialize = get_status != 401;
            let server = MockServer::start().await;
            let resource = format!("{}/mcp", server.uri());
            let prm = format!("{}/custom/metadata", server.uri());
            let rejection = if code == -1 {
                ResponseTemplate::new(status)
            } else {
                let body = json!({
                    "jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message},
                });
                if sse {
                    ResponseTemplate::new(status).set_body_raw(
                        format!(": keepalive\n\nevent: message\ndata: {body}\n\n"),
                        "text/event-stream",
                    )
                } else {
                    ResponseTemplate::new(status).set_body_json(body)
                }
            };
            let challenge = ResponseTemplate::new(401).insert_header(
                "www-authenticate",
                format!("Bearer resource_metadata=\"{prm}\""),
            );
            let post_challenge = challenge.clone();
            Mock::given(method("POST"))
                .and(path("/mcp"))
                .respond_with(move |request: &wiremock::Request| {
                    let body = request.body_json::<Value>().expect("probe");
                    if body["method"] == "server/discover" {
                        return rejection.clone();
                    }
                    assert!(legacy && initialize);
                    assert_eq!(body["method"], "initialize");
                    assert_eq!(body["params"]["protocolVersion"], "2025-06-18");
                    assert!(!request.headers.contains_key("authorization"));
                    post_challenge.clone()
                })
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/mcp"))
                .and(wiremock::matchers::header("accept", "text/event-stream"))
                .respond_with(if initialize {
                    ResponseTemplate::new(get_status)
                } else {
                    challenge
                })
                .expect(u64::from(legacy))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/custom/metadata"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "resource": resource, "authorization_servers": [server.uri()],
                })))
                .expect(u64::from(legacy))
                .mount(&server)
                .await;
            let exchange = request(&resource, /*issuer*/ None);
            let discovered = discover_ema_resource(EmaDiscoveryRequest::from(&exchange)).await;
            assert_eq!(
                discovered.is_ok(),
                legacy,
                "{status} {message}, sse={sse}, get_status={get_status}"
            );
            server.verify().await;
        }
    }
    Ok(())
}

#[test]
fn discovery_rejects_private_or_ambiguous_issuer_destinations() -> Result<()> {
    let resource = Url::parse("https://mcp.example/mcp")?;
    for issuer in [
        "http://issuer.example",
        "https://10.0.0.1",
        "https://127.0.0.1",
        "https://169.254.169.254",
        "https://100.64.0.1",
        "https://198.19.0.1",
        "https://192.0.2.1",
        "https://[::ffff:192.0.2.1]",
        "https://[fd00::1]",
        "https://[::ffff:169.254.169.254]",
        "https://metadata.google.internal",
        "https://metadata.azure.internal",
        "https://test.localhost",
        "https://issuer.example?query=1",
        "https://issuer.example#fragment",
        "https://credentials@issuer.example",
    ] {
        assert!(
            validate_ema_metadata_issuer(&resource, &Url::parse(issuer)?).is_err(),
            "{issuer}"
        );
    }
    validate_ema_metadata_issuer(&resource, &Url::parse("https://issuer.example/tenant")?)?;
    validate_ema_metadata_issuer(
        &Url::parse("http://127.0.0.1/mcp")?,
        &Url::parse("http://[::1]:1234")?,
    )?;
    Ok(())
}

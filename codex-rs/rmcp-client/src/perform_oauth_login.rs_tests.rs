use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use pretty_assertions::assert_eq;
use reqwest::Url;
use serde_json::json;
use std::io::ErrorKind;
use std::net::TcpListener;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::JoinHandle;

use super::CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD;
use super::CallbackOutcome;
use super::OAuthProviderError;
use super::DEFAULT_CIMD_CLIENT_METADATA_URL;
use super::DEFAULT_CIMD_REDIRECT_URI_CALLBACK;
use super::DEFAULT_CIMD_REDIRECT_URI_ROOT;
use super::OAuthCredentialsStoreMode;
use super::append_query_param;
use super::callback_bind_host;
use super::callback_path_from_redirect_uri;
use super::client_id_metadata_document_supported;
use super::parse_oauth_callback;
use super::perform_oauth_login_return_url;
use super::should_rebind_callback_listener_for_cimd_fallback;
use super::should_use_default_cimd_metadata;
use super::validate_callback_listener_settings;
use super::validate_redirect_uri_for_default_cimd_metadata;
use rmcp::transport::auth::AuthorizationMetadata;

fn callback_port_test_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn available_loopback_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral listener should bind");
    listener
        .local_addr()
        .expect("listener should have local addr")
        .port()
}

#[test]
fn parse_oauth_callback_accepts_default_path() {
    let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/callback");
    assert!(matches!(parsed, CallbackOutcome::Success(_)));
}

#[test]
fn parse_oauth_callback_accepts_custom_path() {
    let parsed = parse_oauth_callback("/oauth/callback?code=abc&state=xyz", "/oauth/callback");
    assert!(matches!(parsed, CallbackOutcome::Success(_)));
}

#[test]
fn parse_oauth_callback_rejects_wrong_path() {
    let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/oauth/callback");
    assert!(matches!(parsed, CallbackOutcome::Invalid));
}

#[test]
fn callback_path_comes_from_redirect_uri() {
    let path = callback_path_from_redirect_uri("https://example.com/oauth/callback")
        .expect("redirect URI should parse");
    assert_eq!(path, "/oauth/callback");
}

#[test]
fn callback_bind_host_preserves_ipv6_loopback() {
    let bind_host = callback_bind_host(Some("http://[::1]:33418/callback"));
    assert_eq!(bind_host, "[::1]");
}

#[test]
fn callback_bind_host_preserves_ipv4_loopback_alias() {
    let bind_host = callback_bind_host(Some("http://127.0.0.2:33418/callback"));
    assert_eq!(bind_host, "127.0.0.2");
}

#[test]
fn callback_bind_host_maps_localhost_to_ipv4_loopback() {
    let bind_host = callback_bind_host(Some("http://localhost:33418/callback"));
    assert_eq!(bind_host, "127.0.0.1");
}

#[test]
fn append_query_param_adds_resource_to_absolute_url() {
    let url = append_query_param(
        "https://example.com/authorize?scope=read",
        "resource",
        Some("https://api.example.com"),
    );

    assert_eq!(
        url,
        "https://example.com/authorize?scope=read&resource=https%3A%2F%2Fapi.example.com"
    );
}

#[test]
fn append_query_param_ignores_empty_values() {
    let url = append_query_param(
        "https://example.com/authorize?scope=read",
        "resource",
        Some("   "),
    );

    assert_eq!(url, "https://example.com/authorize?scope=read");
}

#[test]
fn append_query_param_handles_unparseable_url() {
    let url = append_query_param("not a url", "resource", Some("api/resource"));

    assert_eq!(url, "not a url?resource=api%2Fresource");
}

#[test]
fn callback_port_defaults_to_ephemeral_for_non_cimd() {
    let port =
        super::resolve_callback_port(None, false).expect("default callback port should resolve");
    assert_eq!(port.map(std::num::NonZeroU16::get), None);
}

#[test]
fn callback_port_defaults_to_cimd_port_for_cimd_metadata() {
    let port =
        super::resolve_callback_port(None, true).expect("default callback port should resolve");
    assert_eq!(
        port.map(std::num::NonZeroU16::get),
        Some(super::DEFAULT_CIMD_CALLBACK_PORT)
    );
}

#[test]
fn callback_listener_settings_allow_matching_explicit_port() {
    let result =
        validate_callback_listener_settings(Some(33418), Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK));

    assert_eq!(result.is_ok(), true);
}

#[test]
fn callback_listener_settings_reject_conflicting_explicit_port() {
    let err =
        validate_callback_listener_settings(Some(5678), Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK))
            .expect_err("conflicting callback URL and port should be rejected");

    assert!(
        err.to_string()
            .contains("`mcp_oauth_callback_port` is set to `5678`"),
        "unexpected callback listener settings error: {err:#}"
    );
}

#[test]
fn default_cimd_redirect_uri_validation_accepts_supported_uris() {
    let result_root =
        validate_redirect_uri_for_default_cimd_metadata(DEFAULT_CIMD_REDIRECT_URI_ROOT);
    let result_callback =
        validate_redirect_uri_for_default_cimd_metadata(DEFAULT_CIMD_REDIRECT_URI_CALLBACK);

    assert_eq!(result_root.is_ok(), true);
    assert_eq!(result_callback.is_ok(), true);
}

#[test]
fn default_cimd_redirect_uri_validation_rejects_other_uris() {
    let err = validate_redirect_uri_for_default_cimd_metadata("http://127.0.0.1:43210/")
        .expect_err("unexpected success for unsupported redirect URI");

    assert!(
        err.to_string()
            .contains("incompatible with built-in Codex client metadata"),
        "unexpected redirect validation error: {err:#}"
    );
}

#[test]
fn cimd_fallback_skips_rebind_when_existing_listener_is_compatible() {
    let should_rebind =
        should_rebind_callback_listener_for_cimd_fallback(true, DEFAULT_CIMD_REDIRECT_URI_CALLBACK);

    assert_eq!(should_rebind, false);
}

#[test]
fn cimd_fallback_rebinds_when_existing_listener_is_incompatible() {
    let should_rebind =
        should_rebind_callback_listener_for_cimd_fallback(true, "http://127.0.0.1:43210/");

    assert_eq!(should_rebind, true);
}

#[test]
fn cimd_fallback_never_rebinds_when_rebind_not_requested() {
    let should_rebind =
        should_rebind_callback_listener_for_cimd_fallback(false, "http://127.0.0.1:43210/");

    assert_eq!(should_rebind, false);
}

#[test]
fn cimd_support_requires_metadata_flag_and_missing_registration_endpoint() {
    let supported = authorization_metadata(None, true);
    let missing_flag = authorization_metadata(None, false);
    let with_registration = authorization_metadata(Some("https://example.com/register"), true);

    assert_eq!(client_id_metadata_document_supported(&supported), true);
    assert_eq!(should_use_default_cimd_metadata(&supported), true);
    assert_eq!(should_use_default_cimd_metadata(&missing_flag), false);
    assert_eq!(should_use_default_cimd_metadata(&with_registration), false);
}

#[test]
fn parse_oauth_callback_accepts_default_path() {
    let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/callback");
    assert!(matches!(parsed, CallbackOutcome::Success(_)));
}

#[test]
fn parse_oauth_callback_accepts_custom_path() {
    let parsed = parse_oauth_callback("/oauth/callback?code=abc&state=xyz", "/oauth/callback");
    assert!(matches!(parsed, CallbackOutcome::Success(_)));
}

#[test]
fn parse_oauth_callback_rejects_wrong_path() {
    let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/oauth/callback");
    assert!(matches!(parsed, CallbackOutcome::Invalid));
}

#[test]
fn parse_oauth_callback_returns_provider_error() {
    let parsed = parse_oauth_callback(
        "/callback?error=invalid_scope&error_description=scope%20rejected",
        "/callback",
    );

    assert_eq!(
        parsed,
        CallbackOutcome::Error(OAuthProviderError::new(
            Some("invalid_scope".to_string()),
            Some("scope rejected".to_string()),
        ))
    );
}

#[test]
fn callback_path_comes_from_redirect_uri() {
    let path = callback_path_from_redirect_uri("https://example.com/oauth/callback")
        .expect("redirect URI should parse");
    assert_eq!(path, "/oauth/callback");
}

#[test]
fn append_query_param_adds_resource_to_absolute_url() {
    let url = append_query_param(
        "https://example.com/authorize?scope=read",
        "resource",
        Some("https://api.example.com"),
    );

    assert_eq!(
        url,
        "https://example.com/authorize?scope=read&resource=https%3A%2F%2Fapi.example.com"
    );
}

#[test]
fn append_query_param_ignores_empty_values() {
    let url = append_query_param(
        "https://example.com/authorize?scope=read",
        "resource",
        Some("   "),
    );

    assert_eq!(url, "https://example.com/authorize?scope=read");
}

#[test]
fn append_query_param_handles_unparseable_url() {
    let url = append_query_param("not a url", "resource", Some("api/resource"));

    assert_eq!(url, "not a url?resource=api%2Fresource");
}

#[tokio::test]
async fn oauth_login_uses_default_cimd_metadata_when_dynamic_registration_unsupported() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(true, false, false).await;

    let login_handle = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        None,
        None,
    )
    .await
    .expect("oauth login should start with default CIMD metadata URL");
    let (authorization_url, completion) = login_handle.into_parts();

    let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
    let params = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        params.get("client_id").map(std::convert::AsRef::as_ref),
        Some(DEFAULT_CIMD_CLIENT_METADATA_URL)
    );

    let err = completion
        .await
        .expect("oauth completion receiver should resolve")
        .expect_err("oauth should time out in test without callback");
    assert!(
        err.to_string()
            .contains("timed out waiting for OAuth callback"),
        "unexpected oauth completion error: {err}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_rejects_incompatible_callback_for_default_cimd_metadata() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(true, false, false).await;
    let incompatible_port = available_loopback_port();

    let err = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        Some(incompatible_port),
        None,
    )
    .await
    .err()
    .expect("oauth login should fail when callback URI is incompatible with fallback metadata");
    let err_text = format!("{err:#}");

    assert!(
        err_text.contains("incompatible with built-in Codex client metadata"),
        "unexpected oauth setup error: {err:#}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_rejects_conflicting_explicit_callback_url_and_port_for_non_cimd() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(false, true, false).await;

    let err = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        Some(5678),
        Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK),
    )
    .await
    .err()
    .expect("oauth login should fail when callback URL and port conflict");

    assert!(
        err.to_string()
            .contains("`mcp_oauth_callback_port` is set to `5678`"),
        "unexpected oauth setup error: {err:#}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_non_cimd_starts_when_cimd_default_port_is_occupied() {
    let _lock = callback_port_test_lock().lock().await;
    let _occupied_port_listener =
        match TcpListener::bind(("127.0.0.1", super::DEFAULT_CIMD_CALLBACK_PORT)) {
            Ok(listener) => Some(listener),
            Err(err) if err.kind() == ErrorKind::AddrInUse => None,
            Err(err) => panic!("failed to bind default CIMD callback port: {err}"),
        };
    let (server_url, server_handle) = start_oauth_metadata_server(false, true, false).await;

    let login_handle = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        None,
        None,
    )
    .await
    .expect("oauth login should start on an ephemeral callback port");
    let (authorization_url, completion) = login_handle.into_parts();

    let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
    let params = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let redirect_uri = params
        .get("redirect_uri")
        .map(std::convert::AsRef::as_ref)
        .expect("authorization URL should include redirect_uri");
    assert!(
        redirect_uri.starts_with("http://127.0.0.1:"),
        "unexpected redirect URI: {redirect_uri}"
    );
    assert!(
        !redirect_uri.contains(":33418/"),
        "expected non-CIMD redirect URI to avoid default CIMD callback port: {redirect_uri}"
    );

    let err = completion
        .await
        .expect("oauth completion receiver should resolve")
        .expect_err("oauth should time out in test without callback");
    assert!(
        err.to_string()
            .contains("timed out waiting for OAuth callback"),
        "unexpected oauth completion error: {err}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_falls_back_to_default_cimd_metadata_when_registration_fails() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(true, true, true).await;

    let login_handle = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        None,
        None,
    )
    .await
    .expect("oauth login should fall back to default CIMD metadata URL");
    let (authorization_url, completion) = login_handle.into_parts();

    let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
    let params = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        params.get("client_id").map(std::convert::AsRef::as_ref),
        Some(DEFAULT_CIMD_CLIENT_METADATA_URL)
    );
    assert_eq!(
        params.get("redirect_uri").map(std::convert::AsRef::as_ref),
        Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK)
    );

    let err = completion
        .await
        .expect("oauth completion receiver should resolve")
        .expect_err("oauth should time out in test without callback");
    assert!(
        err.to_string()
            .contains("timed out waiting for OAuth callback"),
        "unexpected oauth completion error: {err}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_fallback_rejects_incompatible_explicit_callback_after_registration_failure() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(true, true, true).await;
    let incompatible_port = available_loopback_port();

    let err = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        Some(incompatible_port),
        None,
    )
    .await
    .err()
    .expect("oauth login should fail when explicit callback is incompatible with CIMD fallback");

    assert!(
        err.to_string()
            .contains("incompatible with built-in Codex client metadata"),
        "unexpected oauth setup error: {err:#}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn oauth_login_fallback_rebinds_compatible_explicit_callback_without_port() {
    let _lock = callback_port_test_lock().lock().await;
    let (server_url, server_handle) = start_oauth_metadata_server(true, true, true).await;

    let login_handle = perform_oauth_login_return_url(
        "rmcp-http",
        &server_url,
        OAuthCredentialsStoreMode::File,
        None,
        None,
        &[],
        None,
        Some(1),
        None,
        Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK),
    )
    .await
    .expect("oauth login should start with compatible explicit callback URL");
    let (authorization_url, completion) = login_handle.into_parts();

    let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
    let params = parsed
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        params.get("redirect_uri").map(std::convert::AsRef::as_ref),
        Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK)
    );
    let state = params
        .get("state")
        .map(std::convert::AsRef::as_ref)
        .expect("authorization URL should include state");

    let mut callback_url =
        Url::parse(DEFAULT_CIMD_REDIRECT_URI_CALLBACK).expect("callback URL should parse");
    callback_url
        .query_pairs_mut()
        .append_pair("code", "test-code")
        .append_pair("state", state);
    let callback_response = reqwest::get(callback_url)
        .await
        .expect("callback request should reach local listener");
    assert_eq!(callback_response.status(), StatusCode::OK);
    let callback_response_body = callback_response
        .text()
        .await
        .expect("callback response body should be readable");
    assert_eq!(
        callback_response_body,
        "Authentication complete. You may close this window."
    );

    let err = tokio::time::timeout(Duration::from_secs(5), completion)
        .await
        .expect("oauth completion should resolve after callback")
        .expect("oauth completion receiver should resolve")
        .expect_err("oauth completion should fail in test without token endpoint");
    assert!(
        err.to_string().contains("failed to handle OAuth callback"),
        "unexpected oauth completion error: {err:#}"
    );

    server_handle.abort();
}

fn authorization_metadata(
    registration_endpoint: Option<&str>,
    client_metadata_document_supported: bool,
) -> AuthorizationMetadata {
    let mut additional_fields = serde_json::Map::new();
    additional_fields.insert(
        CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD.to_string(),
        serde_json::Value::Bool(client_metadata_document_supported),
    );

    AuthorizationMetadata {
        authorization_endpoint: "https://example.com/authorize".to_string(),
        token_endpoint: "https://example.com/token".to_string(),
        registration_endpoint: registration_endpoint.map(str::to_string),
        issuer: None,
        jwks_uri: None,
        scopes_supported: None,
        response_types_supported: Some(vec!["code".to_string()]),
        additional_fields: additional_fields.into_iter().collect(),
    }
}

async fn start_oauth_metadata_server(
    client_id_metadata_document_supported: bool,
    include_registration_endpoint: bool,
    registration_fails: bool,
) -> (String, JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let base_url = format!("http://{addr}");
    let mut metadata = json!({
        "authorization_endpoint": format!("{base_url}/oauth/authorize"),
        "token_endpoint": format!("{base_url}/oauth/token"),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "client_id_metadata_document_supported": client_id_metadata_document_supported,
    });
    if include_registration_endpoint && let Some(metadata_obj) = metadata.as_object_mut() {
        metadata_obj.insert(
            "registration_endpoint".to_string(),
            json!(format!("{base_url}/oauth/register")),
        );
    }

    let app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get({
                let metadata = metadata.clone();
                move || async move { Json(metadata.clone()) }
            }),
        )
        .route(
            "/mcp/.well-known/oauth-authorization-server",
            get({
                let metadata = metadata.clone();
                move || async move { Json(metadata.clone()) }
            }),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(move || async move { Json(metadata.clone()) }),
        )
        .route(
            "/oauth/register",
            post(move || async move {
                if registration_fails {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "error": "access_denied",
                            "error_description": "dynamic registration denied by policy",
                        })),
                    );
                }
                (
                    StatusCode::CREATED,
                    Json(json!({
                        "client_id": "codex-test-client-id",
                        "client_secret": null,
                        "client_name": "Codex Test Client",
                        "redirect_uris": [],
                    })),
                )
            }),
        );

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("oauth metadata server should run");
    });

    (format!("{base_url}/mcp"), handle)
}

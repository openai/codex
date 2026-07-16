use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::routing::post;
use codex_app_server_protocol::AppToolSummary;
use codex_app_server_protocol::AppsReadParams;
use codex_app_server_protocol::AppsReadResponse;
use codex_app_server_protocol::ConnectorMetadata;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn app_read_deduplicates_orders_partial_misses_and_reuses_cached_metadata() -> Result<()> {
    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("external@example.com")
            .plan_type("plus")
            .chatgpt_account_id("account-123"),
    )?;
    let state = BatchServerState::new(
        json!({
            "apps": [
                app_response("alpha", "Alpha", Some("https://files.openai.com/content?id=alpha")),
                app_response("beta", "Beta", Some("https://files.openai.com/content?id=beta")),
            ]
        }),
        &access_token,
        "tpp",
    );
    let (server_url, server_handle) = start_batch_server(state.clone()).await?;
    let codex_home = TempDir::new()?;
    write_apps_config(codex_home.path(), &server_url, Some("tpp"))?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let login_id = mcp
        .send_chatgpt_auth_tokens_login_request(
            access_token,
            "account-123".to_string(),
            Some("plus".to_string()),
        )
        .await?;
    let login_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(login_id)),
    )
    .await??;
    assert_eq!(
        to_response::<LoginAccountResponse>(login_response)?,
        LoginAccountResponse::ChatgptAuthTokens {}
    );

    let response = read_apps(
        &mut mcp,
        vec!["beta", "missing", "alpha", "beta", "forbidden"],
        /*include_tools*/ true,
    )
    .await?;
    assert_eq!(
        response,
        AppsReadResponse {
            apps: vec![
                metadata(
                    "beta",
                    "Beta",
                    Some("https://files.openai.com/content?id=beta")
                ),
                metadata(
                    "alpha",
                    "Alpha",
                    Some("https://files.openai.com/content?id=alpha")
                ),
            ],
            missing_app_ids: vec!["missing".to_string(), "forbidden".to_string()],
        }
    );
    assert_eq!(
        state.requests(),
        vec![json!({
            "app_ids": ["beta", "missing", "alpha", "forbidden"],
            "include_tools": true,
        })]
    );

    let cached_response =
        read_apps(&mut mcp, vec!["alpha", "beta"], /*include_tools*/ true).await?;
    assert_eq!(
        cached_response,
        AppsReadResponse {
            apps: vec![
                metadata(
                    "alpha",
                    "Alpha",
                    Some("https://files.openai.com/content?id=alpha")
                ),
                metadata(
                    "beta",
                    "Beta",
                    Some("https://files.openai.com/content?id=beta")
                ),
            ],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(state.requests().len(), 1);

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn app_read_refetches_metadata_only_cache_entries_when_tools_are_requested() -> Result<()> {
    let state = BatchServerState::new(
        json!({
            "apps": [app_response("cached", "Cached", /*icon_url*/ None)]
        }),
        "chatgpt-token",
        "codex",
    );
    let (server_url, server_handle) = start_batch_server(state.clone()).await?;
    let codex_home = TempDir::new()?;
    write_apps_config(
        codex_home.path(),
        &server_url,
        /*apps_mcp_product_sku*/ None,
    )?;
    write_auth(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ false).await?,
        AppsReadResponse {
            apps: vec![metadata_without_tools(
                "cached", "Cached", /*icon_url*/ None
            )],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(
        state.requests(),
        vec![json!({
            "app_ids": ["cached"],
            "include_tools": false,
        })]
    );

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ true).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", /*icon_url*/ None)],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(
        state.requests(),
        vec![
            json!({
                "app_ids": ["cached"],
                "include_tools": false,
            }),
            json!({
                "app_ids": ["cached"],
                "include_tools": true,
            }),
        ]
    );

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ false).await?,
        AppsReadResponse {
            apps: vec![metadata_without_tools(
                "cached", "Cached", /*icon_url*/ None
            )],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ true).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", /*icon_url*/ None)],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(state.requests().len(), 2);

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn app_read_backend_failure_preserves_fresh_cached_records() -> Result<()> {
    let state = BatchServerState::new(
        json!({
            "apps": [app_response("cached", "Cached", /*icon_url*/ None)]
        }),
        "chatgpt-token",
        "codex",
    );
    let (server_url, server_handle) = start_batch_server(state.clone()).await?;
    let codex_home = TempDir::new()?;
    write_apps_config(
        codex_home.path(),
        &server_url,
        /*apps_mcp_product_sku*/ None,
    )?;
    write_auth(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ true).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", /*icon_url*/ None)],
            missing_app_ids: Vec::new(),
        }
    );
    state.set_status(StatusCode::INTERNAL_SERVER_ERROR);

    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: vec!["cached".to_string(), "uncached".to_string()],
            include_tools: true,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(
        error.error.message.contains("failed to read app metadata"),
        "unexpected error: {error:?}"
    );

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"], /*include_tools*/ true).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", /*icon_url*/ None)],
            missing_app_ids: Vec::new(),
        }
    );
    assert_eq!(state.requests().len(), 2);

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn app_read_rejects_more_than_one_hundred_input_ids() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: (0..101).map(|index| format!("app-{index}")).collect(),
            include_tools: false,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.message, "app/read accepts at most 100 appIds");
    Ok(())
}

async fn read_apps(
    mcp: &mut TestAppServer,
    app_ids: Vec<&str>,
    include_tools: bool,
) -> Result<AppsReadResponse> {
    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: app_ids.into_iter().map(str::to_string).collect(),
            include_tools,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn metadata(id: &str, name: &str, icon_url: Option<&str>) -> ConnectorMetadata {
    ConnectorMetadata {
        id: id.to_string(),
        name: name.to_string(),
        description: Some(format!("{name} description")),
        icon_url: icon_url.map(str::to_string),
        tool_summaries: Some(vec![AppToolSummary {
            name: format!("{id}_tool"),
            title: Some(format!("{name} Tool")),
            description: format!("Use {name}"),
        }]),
    }
}

fn metadata_without_tools(id: &str, name: &str, icon_url: Option<&str>) -> ConnectorMetadata {
    ConnectorMetadata {
        tool_summaries: None,
        ..metadata(id, name, icon_url)
    }
}

fn app_response(id: &str, name: &str, icon_url: Option<&str>) -> Value {
    let mut response = json!({
        "id": id,
        "name": name,
        "description": format!("{name} description"),
        "icon_url": null,
        "tools": [{
            "name": format!("{id}_tool"),
            "title": format!("{name} Tool"),
            "description": format!("Use {name}"),
        }],
        "distribution_channel": "ECOSYSTEM_DIRECTORY",
        "branding": {
            "category": "PRODUCTIVITY",
            "developer": "Test Developer",
            "website": "https://example.com",
            "privacy_policy": "https://example.com/privacy",
            "terms_of_service": "https://example.com/terms",
            "is_discoverable_app": true,
        },
        "app_metadata": {
            "review": { "status": "RELEASED" },
            "categories": ["PRODUCTIVITY"],
            "sub_categories": ["CALENDAR"],
            "seo_description": "Search description",
            "screenshots": [{
                "url": "https://example.com/screenshot.png",
                "cdn_url": "must-not-escape",
                "file_id": "file-1",
                "user_prompt": "Use this app",
            }],
            "developer": "Test Developer",
            "version": "1.0.0",
            "version_id": "version-1",
            "version_notes": "Initial release",
            "first_party_type": "test",
            "first_party_requires_install": true,
            "show_in_composer_when_unlinked": true,
            "subtitle": "must-not-escape",
            "mcp_server_instructions": "must-not-escape",
        },
        "labels": null,
        "actions": [{ "name": "must_not_escape_metadata_boundary" }],
        "model_description": "must not escape metadata boundary",
        "icon_assets": { "256_square": "must-not-escape" },
    });
    if let Some(icon_url) = icon_url {
        response["icon_url"] = json!(icon_url);
    }
    response
}

fn write_apps_config(
    codex_home: &Path,
    base_url: &str,
    apps_mcp_product_sku: Option<&str>,
) -> std::io::Result<()> {
    let apps_mcp_product_sku = apps_mcp_product_sku
        .map(|product_sku| format!("apps_mcp_product_sku = \"{product_sku}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"
{apps_mcp_product_sku}

[features]
connectors = true
"#
        ),
    )
}

fn write_auth(codex_home: &Path) -> Result<()> {
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .plan_type("plus"),
        AuthCredentialsStoreMode::File,
    )
}

#[derive(Clone)]
struct BatchServerState {
    requests: Arc<StdMutex<Vec<Value>>>,
    response: Arc<StdMutex<Value>>,
    status: Arc<StdMutex<StatusCode>>,
    access_token: String,
    expected_product_sku: String,
}

impl BatchServerState {
    fn new(response: Value, access_token: &str, expected_product_sku: &str) -> Self {
        Self {
            requests: Arc::new(StdMutex::new(Vec::new())),
            response: Arc::new(StdMutex::new(response)),
            status: Arc::new(StdMutex::new(StatusCode::OK)),
            access_token: access_token.to_string(),
            expected_product_sku: expected_product_sku.to_string(),
        }
    }

    fn requests(&self) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set_status(&self, status: StatusCode) {
        *self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
    }
}

async fn start_batch_server(state: BatchServerState) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let router = Router::new()
        .route("/ps/apps/batch", post(batch_apps))
        .with_state(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://{addr}"), handle))
}

async fn batch_apps(
    State(state): State<BatchServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {}", state.access_token));
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "account-123");
    // Rejecting a mismatch makes both the configured override and default fallback observable.
    let product_sku_ok = headers
        .get("oai-product-sku")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_product_sku);
    if !bearer_ok || !account_ok || !product_sku_ok {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let include_tools = body
        .get("include_tools")
        .and_then(Value::as_bool)
        .unwrap_or_default();
    state
        .requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(body);
    let status = *state
        .status
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if status != StatusCode::OK {
        return Err(status);
    }
    let mut response = state
        .response
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if !include_tools && let Some(apps) = response.get_mut("apps").and_then(Value::as_array_mut) {
        for app in apps {
            app["tools"] = Value::Null;
        }
    }
    Ok(Json(response))
}

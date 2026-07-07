use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::routing::post;
use codex_app_server_protocol::AppBranding;
use codex_app_server_protocol::AppMetadata;
use codex_app_server_protocol::AppReview;
use codex_app_server_protocol::AppScreenshot;
use codex_app_server_protocol::AppsReadParams;
use codex_app_server_protocol::AppsReadResponse;
use codex_app_server_protocol::ConnectorMetadata;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
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
    let state = BatchServerState::new(json!({
        "connectors": [
            connector_response("alpha", "Alpha", Some("https://chatgpt.com/apps/alpha/alpha")),
            connector_response("beta", "Beta", Some("https://chatgpt.com/apps/beta/beta")),
        ]
    }));
    let (server_url, server_handle) = start_batch_server(state.clone()).await?;
    let codex_home = TempDir::new()?;
    write_apps_config(codex_home.path(), &server_url)?;
    write_auth(codex_home.path())?;
    let mut mcp = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let response = read_apps(
        &mut mcp,
        vec!["beta", "missing", "alpha", "beta", "forbidden"],
    )
    .await?;
    assert_eq!(
        response,
        AppsReadResponse {
            apps: vec![
                metadata("beta", "Beta", Some("https://chatgpt.com/apps/beta/beta")),
                metadata(
                    "alpha",
                    "Alpha",
                    Some("https://chatgpt.com/apps/alpha/alpha")
                ),
            ],
            missing_app_ids: vec!["missing".to_string(), "forbidden".to_string()],
        }
    );
    assert_eq!(
        state.requests(),
        vec![json!({
            "connector_ids": ["beta", "missing", "alpha", "forbidden"],
            "include_actions": false,
            "include_model_descriptions": false,
        })]
    );

    let cached_response = read_apps(&mut mcp, vec!["alpha", "beta"]).await?;
    assert_eq!(
        cached_response,
        AppsReadResponse {
            apps: vec![
                metadata(
                    "alpha",
                    "Alpha",
                    Some("https://chatgpt.com/apps/alpha/alpha")
                ),
                metadata("beta", "Beta", Some("https://chatgpt.com/apps/beta/beta")),
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
async fn app_read_backend_failure_preserves_fresh_cached_records() -> Result<()> {
    let state = BatchServerState::new(json!({
        "connectors": [connector_response("cached", "Cached", None)]
    }));
    let (server_url, server_handle) = start_batch_server(state.clone()).await?;
    let codex_home = TempDir::new()?;
    write_apps_config(codex_home.path(), &server_url)?;
    write_auth(codex_home.path())?;
    let mut mcp = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    assert_eq!(
        read_apps(&mut mcp, vec!["cached"]).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", None)],
            missing_app_ids: Vec::new(),
        }
    );
    state.set_status(StatusCode::INTERNAL_SERVER_ERROR);

    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: vec!["cached".to_string(), "uncached".to_string()],
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
        read_apps(&mut mcp, vec!["cached"]).await?,
        AppsReadResponse {
            apps: vec![metadata("cached", "Cached", None)],
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
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: (0..101).map(|index| format!("app-{index}")).collect(),
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

async fn read_apps(mcp: &mut TestAppServer, app_ids: Vec<&str>) -> Result<AppsReadResponse> {
    let request_id = mcp
        .send_apps_read_request(AppsReadParams {
            app_ids: app_ids.into_iter().map(str::to_string).collect(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn metadata(id: &str, name: &str, install_url: Option<&str>) -> ConnectorMetadata {
    ConnectorMetadata {
        id: id.to_string(),
        name: name.to_string(),
        description: Some(format!("{name} description")),
        distribution_channel: Some("ECOSYSTEM_DIRECTORY".to_string()),
        branding: Some(AppBranding {
            category: Some("PRODUCTIVITY".to_string()),
            developer: Some("Test Developer".to_string()),
            website: Some("https://example.com".to_string()),
            privacy_policy: Some("https://example.com/privacy".to_string()),
            terms_of_service: Some("https://example.com/terms".to_string()),
            is_discoverable_app: true,
        }),
        app_metadata: Some(AppMetadata {
            review: Some(AppReview {
                status: "RELEASED".to_string(),
            }),
            categories: Some(vec!["PRODUCTIVITY".to_string()]),
            sub_categories: Some(vec!["CALENDAR".to_string()]),
            seo_description: Some("Search description".to_string()),
            screenshots: Some(vec![AppScreenshot {
                url: Some("https://example.com/screenshot.png".to_string()),
                file_id: Some("file-1".to_string()),
                user_prompt: "Use this app".to_string(),
            }]),
            developer: Some("Test Developer".to_string()),
            version: Some("1.0.0".to_string()),
            version_id: Some("version-1".to_string()),
            version_notes: Some("Initial release".to_string()),
            first_party_type: Some("test".to_string()),
            first_party_requires_install: Some(true),
            show_in_composer_when_unlinked: None,
        }),
        labels: None,
        install_url: install_url.map(str::to_string),
    }
}

fn connector_response(id: &str, name: &str, install_url: Option<&str>) -> Value {
    let mut response = json!({
        "id": id,
        "name": name,
        "description": format!("{name} description"),
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
            "subtitle": "must-not-escape",
            "mcp_server_instructions": "must-not-escape",
        },
        "labels": null,
        "actions": [{ "name": "must_not_escape_metadata_boundary" }],
        "model_description": "must not escape metadata boundary",
        "icon_assets": { "256_square": "must-not-escape" },
    });
    if let Some(install_url) = install_url {
        response["install_url"] = json!(install_url);
    }
    response
}

fn write_apps_config(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"

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
}

impl BatchServerState {
    fn new(response: Value) -> Self {
        Self {
            requests: Arc::new(StdMutex::new(Vec::new())),
            response: Arc::new(StdMutex::new(response)),
            status: Arc::new(StdMutex::new(StatusCode::OK)),
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
        .route("/ps/connectors/batch", post(batch_connectors))
        .with_state(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://{addr}"), handle))
}

async fn batch_connectors(
    State(state): State<BatchServerState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "Bearer chatgpt-token");
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "account-123");
    if !bearer_ok || !account_ok {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
    Ok(Json(
        state
            .response
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
    ))
}

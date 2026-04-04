use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::CodexAvatarAdminAwardGrantParams;
use codex_app_server_protocol::CodexAvatarAdminCapabilitiesReadResponse;
use codex_app_server_protocol::CodexAvatarDefinition;
use codex_app_server_protocol::CodexAvatarEquipParams;
use codex_app_server_protocol::CodexAvatarInventoryReadResponse;
use codex_app_server_protocol::CodexAvatarOwnership;
use codex_app_server_protocol::CodexAvatarRarity;
use codex_app_server_protocol::CodexAvatarStatus;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_core::auth::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn avatar_inventory_read_requires_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_avatar_inventory_read_request().await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "codex account authentication required to manage avatars"
    );
    Ok(())
}

#[tokio::test]
async fn avatar_inventory_read_requires_chatgpt_auth() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let login_request_id = mcp
        .send_login_account_api_key_request("sk-test-key")
        .await?;
    let login_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(login_request_id)),
    )
    .await??;
    let login: LoginAccountResponse = to_response(login_response)?;
    assert_eq!(login, LoginAccountResponse::ApiKey {});

    let request_id = mcp.send_avatar_inventory_read_request().await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.error.message,
        "chatgpt authentication required to manage avatars"
    );
    Ok(())
}

#[tokio::test]
async fn avatar_inventory_read_returns_snapshot() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("GET"))
        .and(path("/api/codex/avatars/inventory"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot_json("prism")))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_avatar_inventory_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: CodexAvatarInventoryReadResponse = to_response(response)?;

    assert_eq!(received, expected_snapshot("prism"));
    Ok(())
}

#[tokio::test]
async fn avatar_equip_forwards_backend_validation_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("POST"))
        .and(path("/api/codex/avatars/equip"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "detail": "cannot equip unowned avatar prism",
        })))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_avatar_equip_request(CodexAvatarEquipParams {
            avatar_id: "prism".to_string(),
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "cannot equip unowned avatar prism");
    Ok(())
}

#[tokio::test]
async fn avatar_equip_returns_snapshot_with_clippy_fallback() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("POST"))
        .and(path("/api/codex/avatars/equip"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot_json("clippy")))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_avatar_equip_request(CodexAvatarEquipParams {
            avatar_id: "sunset".to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: CodexAvatarInventoryReadResponse = to_response(response)?;

    assert_eq!(received, expected_snapshot("clippy"));
    Ok(())
}

#[tokio::test]
async fn avatar_admin_award_forwards_request_and_returns_target_user_snapshot() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("POST"))
        .and(path("/api/codex/avatars/admin/awards"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(body_json(json!({
            "accountUserId": "target-user-456",
            "awardId": "manual-grant-1",
            "avatarId": "prism",
            "sourceType": "manual-admin-grant",
            "sourceRef": "support-ticket-1",
            "awardedAt": 123,
            "awardedBy": "admin-user",
            "metadataJson": "{\"reason\":\"support\"}",
            "sourceSummary": "Manual support grant"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(snapshot_json_for_user("prism", "target-user-456")),
        )
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_avatar_admin_award_request(CodexAvatarAdminAwardGrantParams {
            account_user_id: "target-user-456".to_string(),
            award_id: "manual-grant-1".to_string(),
            avatar_id: "prism".to_string(),
            source_type: "manual-admin-grant".to_string(),
            source_ref: Some("support-ticket-1".to_string()),
            awarded_at: Some(123),
            awarded_by: Some("admin-user".to_string()),
            metadata_json: Some("{\"reason\":\"support\"}".to_string()),
            source_summary: Some("Manual support grant".to_string()),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: CodexAvatarInventoryReadResponse = to_response(response)?;

    assert_eq!(
        received,
        expected_snapshot_for_user("prism", "target-user-456")
    );
    Ok(())
}

#[tokio::test]
async fn avatar_admin_award_forwards_backend_permission_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("POST"))
        .and(path("/api/codex/avatars/admin/awards"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "detail": "Not a Codex admin",
        })))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_avatar_admin_award_request(CodexAvatarAdminAwardGrantParams {
            account_user_id: "target-user-456".to_string(),
            award_id: "manual-grant-1".to_string(),
            avatar_id: "prism".to_string(),
            source_type: "manual-admin-grant".to_string(),
            source_ref: None,
            awarded_at: None,
            awarded_by: None,
            metadata_json: None,
            source_summary: None,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "Not a Codex admin");
    Ok(())
}

#[tokio::test]
async fn avatar_admin_capabilities_read_returns_backend_capability_flags() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;

    let server = MockServer::start().await;
    write_chatgpt_base_url(codex_home.path(), &server.uri())?;
    Mock::given(method("GET"))
        .and(path("/api/codex/avatars/admin/capabilities"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "canGrantAwards": true,
        })))
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new_with_env(codex_home.path(), &[("OPENAI_API_KEY", None)]).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_avatar_admin_capabilities_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: CodexAvatarAdminCapabilitiesReadResponse = to_response(response)?;

    assert_eq!(
        received,
        CodexAvatarAdminCapabilitiesReadResponse {
            can_grant_awards: true,
        }
    );
    Ok(())
}

fn write_chatgpt_base_url(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!("chatgpt_base_url = \"{base_url}\"\n"),
    )
}

fn snapshot_json(equipped_avatar_id: &str) -> Value {
    snapshot_json_for_user(equipped_avatar_id, "account-123")
}

fn snapshot_json_for_user(equipped_avatar_id: &str, account_user_id: &str) -> Value {
    json!({
        "accountUserId": account_user_id,
        "avatarDefinitions": [
            {
                "avatarId": "clippy",
                "slug": "clippy",
                "displayName": "Clippy",
                "description": "The default Codex paperclip avatar",
                "rarity": "common",
                "assetRef": "builtin:clippy",
                "status": "active",
                "sortOrder": 0,
                "createdAt": 0,
                "updatedAt": 0,
            },
            {
                "avatarId": "prism",
                "slug": "prism",
                "displayName": "Prism",
                "description": "A shiny earnable avatar",
                "rarity": "rare",
                "assetRef": "builtin:prism",
                "status": "hidden",
                "sortOrder": 10,
                "createdAt": 1000,
                "updatedAt": 2000,
            }
        ],
        "ownedAvatars": [
            {
                "accountUserId": account_user_id,
                "avatarId": "clippy",
                "firstUnlockedAt": 100,
                "lastAwardedAt": 100,
                "sourceSummary": "Default avatar",
            },
            {
                "accountUserId": account_user_id,
                "avatarId": "prism",
                "firstUnlockedAt": 200,
                "lastAwardedAt": 300,
                "sourceSummary": "Quest reward",
            }
        ],
        "equippedAvatarId": equipped_avatar_id,
        "equippedAt": 400,
        "updatedAt": 500,
        "syncedAt": 600,
        "catalogVersion": 700,
    })
}

fn expected_snapshot(equipped_avatar_id: &str) -> CodexAvatarInventoryReadResponse {
    expected_snapshot_for_user(equipped_avatar_id, "account-123")
}

fn expected_snapshot_for_user(
    equipped_avatar_id: &str,
    account_user_id: &str,
) -> CodexAvatarInventoryReadResponse {
    CodexAvatarInventoryReadResponse {
        account_user_id: account_user_id.to_string(),
        avatar_definitions: vec![
            CodexAvatarDefinition {
                avatar_id: "clippy".to_string(),
                slug: "clippy".to_string(),
                display_name: "Clippy".to_string(),
                description: "The default Codex paperclip avatar".to_string(),
                rarity: CodexAvatarRarity::Common,
                asset_ref: "builtin:clippy".to_string(),
                status: CodexAvatarStatus::Active,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
            },
            CodexAvatarDefinition {
                avatar_id: "prism".to_string(),
                slug: "prism".to_string(),
                display_name: "Prism".to_string(),
                description: "A shiny earnable avatar".to_string(),
                rarity: CodexAvatarRarity::Rare,
                asset_ref: "builtin:prism".to_string(),
                status: CodexAvatarStatus::Hidden,
                sort_order: 10,
                created_at: 1000,
                updated_at: 2000,
            },
        ],
        owned_avatars: vec![
            CodexAvatarOwnership {
                account_user_id: account_user_id.to_string(),
                avatar_id: "clippy".to_string(),
                first_unlocked_at: 100,
                last_awarded_at: 100,
                source_summary: Some("Default avatar".to_string()),
            },
            CodexAvatarOwnership {
                account_user_id: account_user_id.to_string(),
                avatar_id: "prism".to_string(),
                first_unlocked_at: 200,
                last_awarded_at: 300,
                source_summary: Some("Quest reward".to_string()),
            },
        ],
        equipped_avatar_id: equipped_avatar_id.to_string(),
        equipped_at: 400,
        updated_at: 500,
        synced_at: 600,
        catalog_version: Some(700),
    }
}

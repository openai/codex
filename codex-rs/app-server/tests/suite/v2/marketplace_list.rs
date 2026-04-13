use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MarketplaceListParams;
use codex_app_server_protocol::MarketplaceListResponse;
use codex_app_server_protocol::MarketplaceSourceType;
use codex_app_server_protocol::RequestId;
use codex_core::plugins::marketplace_install_root;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn marketplace_list_returns_paginated_configured_marketplaces() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[marketplaces.beta]
last_updated = "2026-04-11T08:00:00Z"
source_type = "git"
source = "git@github.com:acme/beta.git"

[marketplaces.alpha]
last_updated = "2026-04-10T12:34:56Z"
source_type = "git"
source = "https://github.com/acme/alpha.git"
ref = "main"
sparse_paths = ["plugins/alpha"]
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_list_request(MarketplaceListParams {
            cursor: None,
            limit: Some(1),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: MarketplaceListResponse = to_response(response)?;

    assert_eq!(response.data.len(), 1);
    assert_eq!(response.next_cursor.as_deref(), Some("1"));
    assert_eq!(response.data[0].name, "alpha");
    assert_eq!(
        response.data[0].path,
        AbsolutePathBuf::try_from(
            marketplace_install_root(std::fs::canonicalize(codex_home.path())?.as_path())
                .join("alpha")
                .join(".agents/plugins/marketplace.json")
        )?
    );
    assert_eq!(
        response.data[0].last_updated.as_deref(),
        Some("2026-04-10T12:34:56Z")
    );
    assert_eq!(
        response.data[0].source_type,
        Some(MarketplaceSourceType::Git)
    );
    assert_eq!(
        response.data[0].source.as_deref(),
        Some("https://github.com/acme/alpha.git")
    );
    assert_eq!(response.data[0].ref_name.as_deref(), Some("main"));
    assert_eq!(
        response.data[0].sparse_paths,
        vec!["plugins/alpha".to_string()]
    );

    let request_id = mcp
        .send_marketplace_list_request(MarketplaceListParams {
            cursor: response.next_cursor,
            limit: Some(1),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: MarketplaceListResponse = to_response(response)?;

    assert_eq!(response.data.len(), 1);
    assert_eq!(response.next_cursor, None);
    assert_eq!(response.data[0].name, "beta");
    assert_eq!(
        response.data[0].last_updated.as_deref(),
        Some("2026-04-11T08:00:00Z")
    );
    assert_eq!(
        response.data[0].source_type,
        Some(MarketplaceSourceType::Git)
    );
    assert_eq!(
        response.data[0].source.as_deref(),
        Some("git@github.com:acme/beta.git")
    );
    assert_eq!(response.data[0].ref_name, None);
    assert_eq!(response.data[0].sparse_paths, Vec::<String>::new());
    Ok(())
}

#[tokio::test]
async fn marketplace_list_rejects_invalid_cursor() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[marketplaces.alpha]
source_type = "git"
source = "https://github.com/acme/alpha.git"
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_marketplace_list_request(MarketplaceListParams {
            cursor: Some("bad-cursor".to_string()),
            limit: Some(1),
        })
        .await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("invalid cursor"));
    Ok(())
}

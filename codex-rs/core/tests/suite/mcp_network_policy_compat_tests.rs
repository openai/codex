//! Compatibility for trusted MCP servers that predate network policy negotiation.

use anyhow::Context;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_mcp::MCP_NETWORK_POLICY_META_CAPABILITY;
use core_test_support::network_policy_mcp::NetworkPolicyMcp;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn trusted_server_without_policy_support_keeps_caller_metadata() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mcp = NetworkPolicyMcp::start_with_capabilities(json!({})).await?;
    let requirements = CloudConfigBundleFixture::loader_with_enterprise_requirement(
        "[experimental_network]\nenabled = false\ndomains = { 'blocked.example' = 'deny' }",
    );
    let fixture = test_codex()
        .with_exec_server_url(mcp.executor_url.clone())
        .with_cloud_config_bundle(requirements)
        .with_config(|config| {
            let servers = [("legacy".into(), NetworkPolicyMcp::server_config())];
            config
                .mcp_servers
                .set(servers.into())
                .expect("legacy MCP configuration");
        })
        .build_with_remote_and_local_env(&server)
        .await?;
    let result = fixture
        .codex
        .call_mcp_tool(
            "legacy",
            /*environment_id*/ None,
            "network_policy_meta",
            /*arguments*/ None,
            Some(json!({"caller": "keep", "codex/network-policy-meta": {"enabled": true}})),
        )
        .await?;
    let meta = result.structured_content.context("echoed call metadata")?;
    assert_eq!(meta["caller"], "keep");
    assert!(meta.get(MCP_NETWORK_POLICY_META_CAPABILITY).is_none());
    Ok(())
}

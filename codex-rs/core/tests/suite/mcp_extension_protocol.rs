//! Verifies that an extension can choose MCP protocol mode for its own HTTP server.

use std::collections::HashMap;
use std::sync::Arc;

use codex_config::McpServerConfig;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpProtocolMode;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::MockServer;

struct AppsExtensionEndpoint {
    url: String,
    protocol_mode: McpProtocolMode,
}

impl McpServerContributor<Config> for AppsExtensionEndpoint {
    fn id(&self) -> &'static str {
        "apps_protocol_test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            vec![McpServerContribution::SetWithProtocolMode {
                name: "extension_apps".to_string(),
                config: Box::new(
                    serde_json::from_value(json!({ "url": self.url }))
                        .expect("extension Apps MCP config"),
                ),
                protocol_mode: self.protocol_mode,
            }]
        })
    }
}

async fn mcp_methods(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .expect("mock server should capture MCP startup requests")
        .into_iter()
        .filter(|request| request.url.path() == "/api/codex/ps/mcp")
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            body.get("method")?.as_str().map(str::to_string)
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_protocol_mode_does_not_change_other_http_servers() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    for (generic_mode, extension_mode) in [
        (McpProtocolMode::Legacy, McpProtocolMode::V20260728),
        (McpProtocolMode::V20260728, McpProtocolMode::Legacy),
    ] {
        let responses_server = responses::start_mock_server().await;
        let extension_server = responses::start_mock_server().await;
        let extension_url = format!(
            "{}/api/codex/ps/mcp",
            AppsTestServer::mount(&extension_server)
                .await?
                .chatgpt_base_url
        );
        let third_party_server = responses::start_mock_server().await;
        let third_party_url = format!(
            "{}/api/codex/ps/mcp",
            AppsTestServer::mount(&third_party_server)
                .await?
                .chatgpt_base_url
        );
        let mut extensions = ExtensionRegistryBuilder::new();
        extensions.mcp_server_contributor(Arc::new(AppsExtensionEndpoint {
            url: extension_url,
            protocol_mode: extension_mode,
        }));

        let fixture = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_config(move |config| {
                config
                    .features
                    .disable(Feature::Apps)
                    .expect("test config should disable hosted Apps");
                if generic_mode == McpProtocolMode::V20260728 {
                    config
                        .features
                        .enable(Feature::Mcp20260728)
                        .expect("test config should enable generic MCP protocol");
                } else {
                    config
                        .features
                        .disable(Feature::Mcp20260728)
                        .expect("test config should disable generic MCP protocol");
                }
                let third_party: McpServerConfig =
                    serde_json::from_value(json!({ "url": third_party_url }))
                        .expect("third-party MCP config");
                config
                    .mcp_servers
                    .set(HashMap::from([("third_party".to_string(), third_party)]))
                    .expect("test config should accept MCP server");
            })
            .build_with_auto_env(&responses_server)
            .await?;

        wait_for_mcp_server(&fixture.codex, "extension_apps").await?;
        let legacy = vec!["initialize", "notifications/initialized", "tools/list"];
        let modern = vec![
            "server/discover",
            "initialize",
            "notifications/initialized",
            "tools/list",
        ];
        assert_eq!(
            mcp_methods(&extension_server).await,
            if extension_mode == McpProtocolMode::V20260728 {
                modern.clone()
            } else {
                legacy.clone()
            }
        );
        assert_eq!(
            mcp_methods(&third_party_server).await,
            if generic_mode == McpProtocolMode::V20260728 {
                modern
            } else {
                legacy
            }
        );
    }

    Ok(())
}

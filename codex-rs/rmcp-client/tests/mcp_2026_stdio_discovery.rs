use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use futures::FutureExt;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modern_local_and_executor_stdio_discover_metadata_identity_and_catalogs()
-> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_mcp_2026_discovery_stdio_server")?;

    for executor in [false, true] {
        let launcher: Arc<dyn StdioServerLauncher> = if executor {
            Arc::new(ExecutorStdioServerLauncher::new(
                Environment::default_for_tests().get_exec_backend(),
            ))
        } else {
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
        };
        let client = RmcpClient::new_stdio_client_with_protocol_mode(
            server.clone().into(),
            Vec::new(),
            Some(HashMap::from([(
                OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                OsString::from("2026-07-28"),
            )])),
            &[],
            Some(std::env::current_dir()?.to_string_lossy().into_owned()),
            launcher,
            McpProtocolMode::V20260728,
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("stdio-discovery-test", "1.0.0"),
                )
                .with_protocol_version(ProtocolVersion::V_2025_06_18),
                Some(Duration::from_secs(10)),
                Box::new(|_, _| {
                    async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        })
                    }
                    .boxed()
                }),
            )
            .await?;

        let tools = client
            .list_tools(/*params*/ None, Some(Duration::from_secs(10)))
            .await?;
        assert_eq!(tools.tools[0].name.as_ref(), "stdio_echo");
        let resources = client
            .list_resources(/*params*/ None, Some(Duration::from_secs(10)))
            .await?;
        assert_eq!(resources.resources[0].uri, "test://stdio/resource");
        let templates = client
            .list_resource_templates(/*params*/ None, Some(Duration::from_secs(10)))
            .await?;
        assert_eq!(
            templates.resource_templates[0].name,
            "stdio resource template"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_stdio_preserves_existing_protocol_marker_environment() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;

    for executor in [false, true] {
        for version in ["2026-07-28", "1999-01-01"] {
            let launcher: Arc<dyn StdioServerLauncher> = if executor {
                Arc::new(ExecutorStdioServerLauncher::new(
                    Environment::default_for_tests().get_exec_backend(),
                ))
            } else {
                Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
            };
            let client = RmcpClient::new_stdio_client_with_protocol_mode(
                server.clone().into(),
                Vec::new(),
                Some(HashMap::from([(
                    OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                    OsString::from(version),
                )])),
                &[],
                Some(std::env::current_dir()?.to_string_lossy().into_owned()),
                launcher,
                McpProtocolMode::Legacy,
            )
            .await?;

            client
                .initialize(
                    InitializeRequestParams::new(
                        ClientCapabilities::default(),
                        Implementation::new("stdio-legacy-environment-test", "1.0.0"),
                    )
                    .with_protocol_version(ProtocolVersion::V_2025_06_18),
                    Some(Duration::from_secs(10)),
                    Box::new(|_, _| {
                        async {
                            Ok(ElicitationResponse {
                                action: ElicitationAction::Decline,
                                content: None,
                                meta: None,
                            })
                        }
                        .boxed()
                    }),
                )
                .await?;

            let result = client
                .call_tool(
                    "echo".to_string(),
                    Some(json!({
                        "message": "legacy environment",
                        "env_var": "CODEX_MCP_PROTOCOL_VERSION",
                    })),
                    /*meta*/ None,
                    Some(Duration::from_secs(10)),
                )
                .await?;
            assert_eq!(
                result.structured_content,
                Some(json!({
                    "echo": "ECHOING: legacy environment",
                    "env": version,
                }))
            );
            client.shutdown().await;
        }
    }

    Ok(())
}

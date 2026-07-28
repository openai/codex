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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_message_limits_preserve_legacy_local_compatibility() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;

    for (executor, protocol_mode, accepts_oversized) in [
        (false, McpProtocolMode::Legacy, true),
        (false, McpProtocolMode::V20260728, false),
        (true, McpProtocolMode::Legacy, false),
    ] {
        let launcher: Arc<dyn StdioServerLauncher> = if executor {
            Arc::new(ExecutorStdioServerLauncher::new(
                Environment::default_for_tests().get_exec_backend(),
            ))
        } else {
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
        };
        let mut env = HashMap::from([(
            OsString::from("MCP_TEST_OVERSIZED_TOOL_DESCRIPTION"),
            OsString::from("1"),
        )]);
        if protocol_mode == McpProtocolMode::V20260728 {
            env.insert(
                OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                OsString::from("2026-07-28"),
            );
        }
        let client = RmcpClient::new_stdio_client_with_protocol_mode(
            server.clone().into(),
            Vec::new(),
            Some(env),
            &[],
            Some(std::env::current_dir()?.to_string_lossy().into_owned()),
            launcher,
            protocol_mode,
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("stdio-limit-test", "1.0.0"),
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
            .list_tools(/*params*/ None, Some(Duration::from_secs(10)))
            .await;
        assert_eq!(
            result.is_ok(),
            accepts_oversized,
            "unexpected stdio size handling (executor={executor}, mode={protocol_mode:?})"
        );
        client.shutdown().await;
    }
    Ok(())
}

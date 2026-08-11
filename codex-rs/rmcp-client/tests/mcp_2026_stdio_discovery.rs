use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

#[test]
fn local_stdio_inherits_ca_certificate_variables() -> anyhow::Result<()> {
    let directories = tempfile::tempdir()?;
    let source_dir = directories.path().join("source");
    let server_dir = directories.path().join("server");
    std::fs::create_dir_all(source_dir.join("certs"))?;
    std::fs::create_dir_all(&server_dir)?;

    #[cfg(windows)]
    let requests_ca_bundle = match source_dir.components().next() {
        Some(std::path::Component::Prefix(prefix)) => match prefix.kind() {
            std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) => {
                format!("{}:certs\\custom-ca.pem", char::from(drive))
            }
            _ => "certs/custom-ca.pem".to_string(),
        },
        _ => "certs/custom-ca.pem".to_string(),
    };
    #[cfg(not(windows))]
    let requests_ca_bundle = "certs/custom-ca.pem";

    let output = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("local_stdio_inherits_ca_certificate_variables_child")
        .arg("--ignored")
        .current_dir(&source_dir)
        .envs(
            CUSTOM_CA_ENV_KEYS
                .into_iter()
                .map(|name| (name, "certs/custom-ca.pem")),
        )
        .env("CODEX_CA_CERTIFICATE", "")
        .env("REQUESTS_CA_BUNDLE", requests_ca_bundle)
        .env("CODEX_MCP_TEST_SERVER_CWD", &server_dir)
        .output()?;

    assert!(
        output.status.success(),
        "MCP subprocess inheritance test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "child process for local_stdio_inherits_ca_certificate_variables"]
async fn local_stdio_inherits_ca_certificate_variables_child() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;
    let source_dir = std::env::current_dir()?;
    let server_dir = std::env::var("CODEX_MCP_TEST_SERVER_CWD")?;
    let expected = source_dir.join("certs").join("custom-ca.pem");
    let npm_override = HashMap::from([(
        OsString::from("NPM_CONFIG_CAFILE"),
        OsString::from("explicit/custom-ca.pem"),
    )]);

    for (overrides, env_name, expected) in [
        (
            None,
            "SSL_CERT_FILE",
            Some(expected.to_string_lossy().into_owned()),
        ),
        (
            None,
            "REQUESTS_CA_BUNDLE",
            Some(expected.to_string_lossy().into_owned()),
        ),
        (None, "CODEX_CA_CERTIFICATE", None),
        (
            Some(npm_override.clone()),
            "NPM_CONFIG_CAFILE",
            Some("explicit/custom-ca.pem".to_string()),
        ),
        (Some(npm_override), "npm_config_cafile", None),
    ] {
        let client = RmcpClient::new_stdio_client(
            server.clone().into(),
            Vec::new(),
            overrides,
            &[],
            Some(server_dir.clone()),
            Arc::new(LocalStdioServerLauncher::new(source_dir.clone())),
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("stdio-ca-inheritance-test", "1.0.0"),
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
                Some(json!({ "message": "ca inheritance", "env_var": env_name })),
                /*meta*/ None,
                Some(Duration::from_secs(10)),
            )
            .await?;
        assert_eq!(
            result.structured_content,
            Some(json!({ "echo": "ECHOING: ca inheritance", "env": expected }))
        );
        client.shutdown().await;
    }

    Ok(())
}

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

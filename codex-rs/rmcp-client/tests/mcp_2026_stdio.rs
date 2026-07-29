use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_rmcp_client::Elicitation;
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
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

async fn exercise_stdio_server(
    server_mode: &str,
    protocol_mode: McpProtocolMode,
    opt_in: bool,
    use_executor: bool,
) -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_mcp_2026_stdio_server")?;
    let launcher: Arc<dyn StdioServerLauncher> = if use_executor {
        Arc::new(ExecutorStdioServerLauncher::new(
            Environment::default_for_tests().get_exec_backend(),
        ))
    } else {
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
    };
    let env = opt_in.then(|| {
        HashMap::from([(
            OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
            OsString::from("2026-07-28"),
        )])
    });
    let client = RmcpClient::new_stdio_client_with_protocol_mode(
        server.into(),
        vec![OsString::from(server_mode)],
        env,
        &[],
        /*cwd*/ Some(std::env::current_dir()?.to_string_lossy().into_owned()),
        launcher,
        protocol_mode,
    )
    .await?;

    let mut capabilities = ClientCapabilities::default();
    capabilities.elicitation =
        Some(ElicitationCapability::new().with_form(FormElicitationCapability::new()));
    let elicitation_count = Arc::new(AtomicUsize::new(0));
    let observed_elicitations = Arc::clone(&elicitation_count);
    let legacy_session = server_mode.starts_with("legacy");
    let initialized = client
        .initialize(
            InitializeRequestParams::new(
                capabilities,
                Implementation::new("codex-mcp-client", "0.0.0-test"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            Some(Duration::from_secs(5)),
            Box::new(move |_request_id, request| {
                let observed_elicitations = Arc::clone(&observed_elicitations);
                async move {
                    observed_elicitations.fetch_add(1, Ordering::Relaxed);
                    let Elicitation::Mcp(ElicitRequestParams::FormElicitationParams {
                        requested_schema,
                        ..
                    }) = request
                    else {
                        anyhow::bail!("expected a standard MCP form elicitation");
                    };
                    let content = if legacy_session {
                        assert_eq!(
                            serde_json::to_value(requested_schema)?,
                            json!({
                                "type": "object",
                                "properties": {
                                    "name": {"type": "string", "default": "John Doe"},
                                    "age": {"type": "integer", "default": 30},
                                    "score": {"type": "number", "default": 95.5},
                                    "status": {
                                        "type": "string",
                                        "enum": ["active", "inactive"],
                                        "default": "active",
                                    },
                                    "verified": {"type": "boolean", "default": true},
                                },
                                "required": [],
                            }),
                        );
                        json!({
                            "name": "John Doe",
                            "age": 30,
                            "score": 95.5,
                            "status": "active",
                            "verified": true,
                        })
                    } else {
                        json!({"approved": true})
                    };
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(content),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    let expected_version = if legacy_session {
        ProtocolVersion::V_2025_06_18
    } else {
        ProtocolVersion::V_2026_07_28
    };
    assert_eq!(initialized.protocol_version, expected_version);
    assert_eq!(
        initialized
            .server_info
            .as_ref()
            .map(|server_info| server_info.name.as_str()),
        Some(if legacy_session {
            "legacy-stdio-test"
        } else {
            "strict-stdio-test"
        })
    );
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    assert_eq!(
        tools
            .tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        vec!["echo"]
    );
    let result = client
        .call_tool(
            "echo".to_owned(),
            Some(json!({"message": "hello stdio"})),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await?;
    assert_eq!(
        result.content[0].as_text().map(|text| text.text.as_str()),
        Some(if legacy_session {
            "legacy approved"
        } else {
            "modern approved"
        })
    );
    assert_eq!(elicitation_count.load(Ordering::Relaxed), 1);
    client.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modern_local_stdio_discovers_metadata_only_identity_and_drives_mrtr() -> anyhow::Result<()>
{
    exercise_stdio_server(
        "modern",
        McpProtocolMode::V20260728,
        /*opt_in*/ true,
        /*use_executor*/ false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modern_executor_stdio_discovers_metadata_only_identity_and_drives_mrtr()
-> anyhow::Result<()> {
    exercise_stdio_server(
        "modern",
        McpProtocolMode::V20260728,
        /*opt_in*/ true,
        /*use_executor*/ true,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_stdio_supports_sep1034_defaults_without_the_rollout_flag() -> anyhow::Result<()> {
    exercise_stdio_server(
        "legacy",
        McpProtocolMode::Legacy,
        /*opt_in*/ false,
        /*use_executor*/ false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollout_flag_alone_preserves_legacy_stdio_and_sep1034_defaults() -> anyhow::Result<()> {
    exercise_stdio_server(
        "legacy",
        McpProtocolMode::V20260728,
        /*opt_in*/ false,
        /*use_executor*/ false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modern_stdio_safely_falls_back_to_legacy_elicitation() -> anyhow::Result<()> {
    exercise_stdio_server(
        "legacy-fallback",
        McpProtocolMode::V20260728,
        /*opt_in*/ true,
        /*use_executor*/ false,
    )
    .await
}

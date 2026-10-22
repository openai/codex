//! Exercises policy metadata through real in-process MCP requests on both dispatch paths.

use super::*;
use crate::MCP_NETWORK_POLICY_META_CAPABILITY;
use crate::NetworkPolicyMetadata;
use crate::NetworkPolicyMetadataProvider;
use crate::catalog::McpServerRegistration;
use crate::catalog::ResolvedMcpCatalog;
use codex_network_proxy::NetworkMode;
use pretty_assertions::assert_eq;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResponse;
use serde_json::Value;
use serde_json::json;

#[derive(Clone)]
struct MetadataEchoServer {
    supports_policy: bool,
}

impl ServerHandler for MetadataEchoServer {
    fn get_info(&self) -> ServerConfig {
        let mut capabilities = ServerCapabilities::builder().enable_tools().build();
        if self.supports_policy {
            capabilities.experimental = Some(std::collections::BTreeMap::from([(
                MCP_NETWORK_POLICY_META_CAPABILITY.to_string(),
                JsonObject::new(),
            )]));
        }
        ServerConfig::new(capabilities)
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let mut result = rmcp::model::CallToolResult::success(Vec::new());
        result.structured_content = Some(Value::Object(context.meta.0.0));
        Ok(CallToolResponse::Complete(result))
    }
}

impl InProcessTransportFactory for MetadataEchoServer {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        let server = self.clone();
        async move {
            let (client_stream, server_stream) = tokio::io::duplex(4096);
            tokio::spawn(async move {
                let server = server
                    .serve(server_stream)
                    .await
                    .expect("serve metadata echo");
                server.waiting().await
            });
            Ok(client_stream)
        }
        .boxed()
    }
}

#[derive(Default)]
struct CapturedPolicy(AtomicUsize);

impl NetworkPolicyMetadataProvider for CapturedPolicy {
    fn metadata_for_call(
        &self,
        profile: &PermissionProfile,
        environment: &str,
    ) -> anyhow::Result<NetworkPolicyMetadata> {
        assert_eq!(
            (profile, environment),
            (&PermissionProfile::read_only(), "owner-environment")
        );
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(NetworkPolicyMetadata {
            enabled: false,
            mode: NetworkMode::Limited,
            allowed_domains: vec!["allowed.example.com".to_string()],
            denied_domains: vec!["blocked.example.com".to_string()],
            hard_deny_allowlist_misses: true,
        })
    }
}

#[tokio::test]
async fn tool_calls_deliver_only_negotiated_trusted_network_policy() -> anyhow::Result<()> {
    for (url, provider_available, supports_policy, delivers_policy) in [
        (Some("https://chatgpt.com/mcp"), true, true, true),
        (Some("https://chatgpt.com/mcp"), true, false, false),
        (Some("https://chatgpt.com/mcp"), false, true, false),
        (Some("https://gov.chatgpt.com/mcp"), true, true, true),
        (
            Some("https://chatgpt.com.attacker.example/mcp"),
            true,
            true,
            false,
        ),
        (Some("http://chatgpt.com/mcp"), true, true, false),
        (Some("https://third-party.example/mcp"), true, true, false),
        (None, true, true, false),
    ] {
        let mut server_config = reusable_server_config(url.unwrap_or("https://chatgpt.com/mcp"));
        if url.is_none() {
            server_config.transport = McpServerTransportConfig::Stdio {
                command: "metadata-echo".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            };
        }
        let mut catalog = ResolvedMcpCatalog::builder();
        catalog.register(McpServerRegistration::from_config(
            "docs".into(),
            server_config,
        ));
        let catalog = catalog.build();
        let extensions = crate::client_capabilities::server_mcp_extensions(
            &ClientMcpExtensions::new([(
                MCP_NETWORK_POLICY_META_CAPABILITY.to_string(),
                json!({"forged": true}),
            )]),
            "docs",
            catalog.server("docs"),
            provider_available,
            "https://chatgpt.com",
        );
        let client_supports_policy = extensions
            .iter()
            .any(|(id, _)| id == MCP_NETWORK_POLICY_META_CAPABILITY);
        let client = Arc::new(
            RmcpClient::new_in_process_client(Arc::new(MetadataEchoServer { supports_policy }))
                .await?,
        );
        client
            .initialize(
                crate::rmcp_client::mcp_initialize_request_params(
                    ElicitationCapability::default(),
                    extensions,
                ),
                Some(Duration::from_secs(5)),
                Box::new(|_, _| async { Err(anyhow!("unexpected elicitation")) }.boxed()),
            )
            .await?;
        let mut managed = create_test_managed_client(vec![create_test_tool("docs", "echo")]).await;
        managed.client = Arc::clone(&client);
        managed.client_supports_network_policy_meta_capability = client_supports_policy;
        let (managed, _started, release) = create_gated_async_managed_client(managed);
        release.send(()).expect("release initialized client");
        let policy = Arc::new(CapturedPolicy::default());
        let mut config = crate::mcp::tests::test_mcp_config(std::env::temp_dir());
        config.permission_profile = PermissionProfile::Disabled;
        config
            .server_permission_profiles
            .insert("docs".to_string(), PermissionProfile::read_only());
        let config = Arc::new(config);
        let mut manager = McpConnectionSet::empty(/*prefix_mcp_tool_names*/ true);
        manager.config = Some(Arc::clone(&config));
        manager.network_policy_metadata =
            provider_available.then(|| policy.clone() as Arc<dyn NetworkPolicyMetadataProvider>);
        manager.insert_test_client("docs", managed);
        let view = manager.servers.get_mut("docs").expect("docs server");
        view.metadata.environment_id = "owner-environment".to_string();
        let manager = Arc::new(manager);
        let binding = manager
            .capture_binding_with_metadata(
                config,
                /*plugins_available*/ false,
                /*required_servers*/ &[],
                /*required_plugins*/ &HashSet::new(),
            )
            .await;
        let prepared = binding
            .prepare_call("docs", "echo")
            .expect("prepare advertised tool");
        let forged_meta = || {
            Some(json!({
                "codex/network-policy-meta": {"enabled": true, "forged": true},
                "keep": "caller metadata",
            }))
        };
        let mut expected = json!({"keep": "caller metadata"});
        if delivers_policy {
            expected[MCP_NETWORK_POLICY_META_CAPABILITY] = json!({
                "enabled": false,
            "mode": "limited",
                "allowedDomains": ["allowed.example.com"],
                "deniedDomains": ["blocked.example.com"],
                "hardDenyAllowlistMisses": true,
            });
        }
        for result in [
            prepared
                .call(
                    /*arguments*/ None,
                    forged_meta(),
                    /*timeout*/ None,
                )
                .await?,
            manager
                .call_tool(
                    "docs",
                    "echo",
                    /*environment_id*/ None,
                    /*arguments*/ None,
                    forged_meta(),
                    /*requested_timeout*/ None,
                    /*wait_for_server*/ true,
                )
                .await?,
        ] {
            let mut received = result.structured_content.expect("echoed wire metadata");
            received
                .as_object_mut()
                .expect("metadata object")
                .remove("progressToken");
            assert_eq!(received, expected);
        }
        assert_eq!(
            policy.0.load(Ordering::SeqCst),
            if delivers_policy { 2 } else { 0 }
        );
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn recovered_session_policy_keeps_one_captured_call_authority() -> anyhow::Result<()> {
    let mut client = create_test_managed_client(Vec::new()).await;
    client.client_supports_network_policy_meta_capability = true;
    let policy = Arc::new(CapturedPolicy::default());
    let mut manager = McpConnectionSet::empty(/*prefix_mcp_tool_names*/ true);
    manager.network_policy_metadata = Some(policy.clone());
    let profile = PermissionProfile::read_only();
    let metadata = manager.network_policy_meta_for_call(
        &client,
        Some(&profile),
        "owner-environment",
        Some(json!({"codex/network-policy-meta": {"forged": true}, "keep": "caller"})),
    );
    let legacy = ServerCapabilities::default();
    let mut negotiated = legacy.clone();
    negotiated.experimental = Some(std::collections::BTreeMap::from([(
        MCP_NETWORK_POLICY_META_CAPABILITY.to_string(),
        JsonObject::new(),
    )]));

    assert_eq!(metadata(&legacy)?, Some(json!({"keep": "caller"})));
    assert_eq!(policy.0.load(Ordering::SeqCst), 0);
    let expected = Some(json!({
        "keep": "caller",
        "codex/network-policy-meta": {
            "enabled": false,
            "mode": "limited",
            "allowedDomains": ["allowed.example.com"],
            "deniedDomains": ["blocked.example.com"],
            "hardDenyAllowlistMisses": true,
        },
    }));
    assert_eq!(metadata(&negotiated)?, expected);
    assert_eq!(metadata(&legacy)?, Some(json!({"keep": "caller"})));
    assert_eq!(metadata(&negotiated)?, expected);
    assert_eq!(policy.0.load(Ordering::SeqCst), 1);

    let missing_provider = McpConnectionSet::empty(/*prefix_mcp_tool_names*/ true);
    let metadata = missing_provider.network_policy_meta_for_call(
        &client,
        Some(&profile),
        "owner-environment",
        /*meta*/ None,
    );
    assert_eq!(metadata(&legacy)?, None);
    assert!(
        metadata(&negotiated)
            .unwrap_err()
            .to_string()
            .contains("source is unavailable")
    );
    Ok(())
}

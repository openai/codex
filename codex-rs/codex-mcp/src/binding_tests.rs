use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_config::AppToolApproval;
use codex_config::Constrained;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_rmcp_client::InProcessTransportFactory;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Tool;
use tokio::io::DuplexStream;
use tokio::sync::Notify;

use super::McpBinding;
use super::PreparedMcpCall;
use crate::binding_clients::McpBindingClients;
use crate::connection_manager::McpConnectionManager;
use crate::rmcp_client::ManagedClient;
use crate::server::McpServerMetadata;
use crate::server::McpServerOrigin;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;

const SERVER_NAME: &str = "docs";
const TOOL_NAME: &str = "search";

struct TestInProcessTransportFactory;

impl InProcessTransportFactory for TestInProcessTransportFactory {
    fn open(&self) -> futures::future::BoxFuture<'static, io::Result<DuplexStream>> {
        async {
            let (client_stream, _server_stream) = tokio::io::duplex(1);
            Ok(client_stream)
        }
        .boxed()
    }
}

struct TestStep {
    step: Arc<McpBinding>,
    client: Arc<RmcpClient>,
    tool_catalog_revision: Arc<tokio::sync::RwLock<u64>>,
}

async fn test_step(
    label: &str,
    approval_mode: AppToolApproval,
    supports_sandbox_state_meta: bool,
) -> TestStep {
    let tool = ToolInfo {
        server_name: SERVER_NAME.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: TOOL_NAME.to_string(),
        callable_namespace: SERVER_NAME.to_string(),
        namespace_description: None,
        tool: Tool::new(
            TOOL_NAME.to_string(),
            format!("{label} catalog"),
            Arc::new(JsonObject::default()),
        ),
        openai_file_input_optional_fields: Default::default(),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    };
    let client = Arc::new(
        RmcpClient::new_in_process_client(Arc::new(TestInProcessTransportFactory))
            .await
            .expect("create in-process MCP client"),
    );
    let managed_client = Arc::new(ManagedClient {
        client: Arc::clone(&client),
        server_info: McpServerInfo {
            name: label.to_string(),
            title: Some(format!("{label} server")),
            version: "1.0.0".to_string(),
            description: None,
            icons: None,
            website_url: None,
        },
        tools: vec![tool.clone()],
        tool_filter: ToolFilter::default(),
        tool_timeout: None,
        server_instructions: None,
        server_supports_sandbox_state_meta_capability: supports_sandbox_state_meta,
        codex_apps_tools_cache_context: None,
    });
    let clients = Arc::new(McpBindingClients::new(HashMap::from([(
        SERVER_NAME.to_string(),
        Arc::clone(&managed_client),
    )])));
    let connections = Arc::new(
        McpConnectionManager::new_uninitialized_with_permission_profile(
            &Constrained::allow_any(AskForApproval::OnRequest),
            &PermissionProfile::default(),
            /*prefix_mcp_tool_names*/ true,
        ),
    );
    let tool_catalog_revision = Arc::new(tokio::sync::RwLock::new(0));
    let prepared = PreparedMcpCall::new(
        Arc::clone(&connections),
        managed_client,
        /*catalog_revision*/ 0,
        Arc::clone(&tool_catalog_revision),
        tool.clone(),
        McpServerMetadata {
            environment_id: format!("{label}-environment"),
            pollutes_memory: label == "old",
            origin: Some(McpServerOrigin::StreamableHttp(format!(
                "https://{label}.example"
            ))),
            supports_parallel_tool_calls: false,
            default_tools_approval_mode: Some(approval_mode),
            tool_approval_modes: HashMap::new(),
        },
        Some(format!("{label}-plugin")),
        label == "old",
    );
    let calls = HashMap::from([((SERVER_NAME.to_string(), TOOL_NAME.to_string()), prepared)]);

    TestStep {
        step: Arc::new(McpBinding::new(
            connections,
            clients,
            Arc::new(crate::mcp::tests::test_mcp_config(std::env::temp_dir())),
            /*plugins_available*/ false,
            vec![tool],
            calls,
        )),
        client,
        tool_catalog_revision,
    }
}

#[tokio::test]
async fn prepared_call_keeps_captured_connection_and_authority_after_refresh() -> anyhow::Result<()>
{
    let old = test_step(
        "old",
        AppToolApproval::Prompt,
        /*supports_sandbox_state_meta*/ true,
    )
    .await;
    let old_call = old
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("old step should prepare the advertised tool");
    let old_connections = Arc::downgrade(&old.step.connections);

    let new = test_step(
        "new",
        AppToolApproval::Approve,
        /*supports_sandbox_state_meta*/ false,
    )
    .await;
    let new_call = new
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("new step should prepare the advertised tool");

    assert_eq!(
        (
            old.step.tools()[0].tool.description.as_deref(),
            old_call.tool_info().tool.description.as_deref(),
            old_call.server_origin(),
            old_call.server_environment_id(),
            old_call.server_pollutes_memory(),
            old_call.tool_approval_mode(),
            old_call.plugin_id(),
            old_call.is_selected_plugin_server(),
            old_call
                .server_supports_sandbox_state_meta_capability()
                .await?,
        ),
        (
            Some("old catalog"),
            Some("old catalog"),
            Some("https://old.example"),
            "old-environment",
            true,
            AppToolApproval::Prompt,
            Some("old-plugin"),
            true,
            true,
        )
    );
    assert_eq!(
        (
            new.step.tools()[0].tool.description.as_deref(),
            new_call.tool_info().tool.description.as_deref(),
            new_call.server_environment_id(),
            new_call.tool_approval_mode(),
        ),
        (
            Some("new catalog"),
            Some("new catalog"),
            "new-environment",
            AppToolApproval::Approve,
        )
    );
    assert!(Arc::ptr_eq(&old_call.client.client, &old.client));
    assert!(!Arc::ptr_eq(&old.client, &new.client));

    drop(old.step);
    assert!(
        old_connections.upgrade().is_some(),
        "the prepared call should keep its captured connection set alive"
    );
    drop(old_call);
    assert!(
        old_connections.upgrade().is_none(),
        "the captured connection set should be released with the prepared call"
    );
    Ok(())
}

#[tokio::test]
async fn prepared_call_does_not_reroute_after_captured_connection_closes() {
    let old = test_step(
        "old",
        AppToolApproval::Prompt,
        /*supports_sandbox_state_meta*/ true,
    )
    .await;
    let old_call = old
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("old step should prepare the advertised tool");
    let new = test_step(
        "new",
        AppToolApproval::Approve,
        /*supports_sandbox_state_meta*/ false,
    )
    .await;
    assert!(!Arc::ptr_eq(&old.client, &new.client));

    old.client.shutdown().await;

    let error = old_call
        .call(
            Some(serde_json::json!({"query": "codex"})),
            /*meta*/ None,
        )
        .await
        .expect_err("a call bound to a closed connection must fail");
    assert!(
        format!("{error:#}").contains("MCP client is shut down"),
        "the prepared call should fail on its captured client: {error:#}"
    );
}

#[tokio::test]
async fn prepared_call_is_rejected_after_catalog_refresh() {
    let step = test_step(
        "old",
        AppToolApproval::Prompt,
        /*supports_sandbox_state_meta*/ true,
    )
    .await;
    let prepared = step
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("step should prepare the advertised tool");

    *step.tool_catalog_revision.write().await += 1;

    let error = prepared
        .call(
            Some(serde_json::json!({"query": "codex"})),
            /*meta*/ None,
        )
        .await
        .expect_err("a call from an older catalog must be rejected");
    assert!(
        format!("{error:#}").contains("catalog changed"),
        "unexpected error: {error:#}"
    );
}

#[tokio::test]
async fn stale_prepared_call_does_not_run_preparation() {
    let step = test_step(
        "old",
        AppToolApproval::Prompt,
        /*supports_sandbox_state_meta*/ true,
    )
    .await;
    let prepared = step
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("step should prepare the advertised tool");
    *step.tool_catalog_revision.write().await += 1;
    let prepared_side_effect_ran = Arc::new(AtomicBool::new(false));
    let marker = Arc::clone(&prepared_side_effect_ran);

    prepared
        .call_with_preparation(|| async move {
            marker.store(true, Ordering::SeqCst);
            Ok((None, None))
        })
        .await
        .expect_err("a call from an older catalog must be rejected");

    assert!(!prepared_side_effect_ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn preparation_holds_catalog_authority_until_it_finishes() {
    let step = test_step(
        "old",
        AppToolApproval::Prompt,
        /*supports_sandbox_state_meta*/ true,
    )
    .await;
    let prepared = step
        .step
        .prepare_call(SERVER_NAME, TOOL_NAME)
        .expect("step should prepare the advertised tool");
    let preparation_started = Arc::new(Notify::new());
    let finish_preparation = Arc::new(Notify::new());
    let started = Arc::clone(&preparation_started);
    let finish = Arc::clone(&finish_preparation);
    let call = tokio::spawn(async move {
        prepared
            .call_with_preparation(|| async move {
                started.notify_one();
                finish.notified().await;
                Err(anyhow::anyhow!("stop after preparation"))
            })
            .await
    });

    preparation_started.notified().await;
    assert!(
        step.tool_catalog_revision.try_write().is_err(),
        "catalog replacement must wait for irreversible call preparation"
    );
    finish_preparation.notify_one();
    call.await
        .expect("call task should finish")
        .expect_err("the test preparation should stop the call");
    assert!(step.tool_catalog_revision.try_write().is_ok());
}

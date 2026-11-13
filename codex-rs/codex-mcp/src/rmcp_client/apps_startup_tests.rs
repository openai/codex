//! Apps tool discovery across concurrent startups on independent MCP connections.
#![cfg(test)]

use super::*;
use crate::client_tool_catalog::ToolCatalogSnapshot;
use crate::connection_manager::tests::create_test_apps_client;
use crate::connection_manager::tests::create_test_tool;
use crate::mcp::tests::test_mcp_config;
use codex_connectors::ConnectorRuntimeContextKey;
use codex_connectors::ConnectorRuntimeManager;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::atomic::AtomicUsize;
use tokio::sync::Notify;

fn tool_names(catalog: ToolCatalogSnapshot) -> Vec<String> {
    catalog
        .tools
        .iter()
        .map(|tool| tool.tool.name.to_string())
        .collect()
}

struct AppsStartup {
    task: tokio::task::JoinHandle<Result<ManagedClient, StartupOutcomeError>>,
    list_started: Arc<Notify>,
    release_list: Arc<Notify>,
    list_requests: Arc<AtomicUsize>,
}

async fn start_overlapping_apps_clients(
    home: &Path,
    startup_timeout: Duration,
) -> anyhow::Result<(AppsStartup, AppsStartup)> {
    let cache = ConnectorRuntimeManager::new_without_cache().context(
        home.to_path_buf(),
        ConnectorRuntimeContextKey::personal(Some("account".into()), Some("user".into())),
    );
    let elicitation_requests = ElicitationRequestManager::new(
        Arc::new(test_mcp_config(home.to_path_buf())),
        /*reviewer*/ None,
        /*lifecycle*/ None,
        Default::default(),
    );
    let start = |tool_name: &str| {
        let (publish, mut initialized) = watch::channel(None);
        let params = StartServerTaskParams {
            server_key: McpServerKey::unscoped(CODEX_APPS_MCP_SERVER_NAME),
            publish_apps: Some(publish),
            sharing_declined: None,
            server_capabilities: Arc::default(),
            is_codex_apps_mcp_server: true,
            startup_timeout,
            tx_event: None,
            elicitation_requests: elicitation_requests.clone(),
            codex_apps_tools_cache_context: Some(cache.clone()),
            tool_catalog_cache_context: None,
            tool_catalog_fetch_ticket: None,
            client_elicitation_capability: ElicitationCapability::default(),
            client_mcp_extensions: ClientMcpExtensions::default(),
            auth_changes: None,
            catalog_item_limit: crate::pagination::MAX_CODEX_APPS_TOOL_CATALOG_ITEMS,
        };
        let tool = create_test_tool(CODEX_APPS_MCP_SERVER_NAME, tool_name).tool;
        async move {
            let list_started = Arc::new(Notify::new());
            let release_list = Arc::new(Notify::new());
            let list_requests = Arc::new(AtomicUsize::new(0));
            let client = create_test_apps_client(
                tool,
                Some(Arc::clone(&list_started)),
                Some(Arc::clone(&release_list)),
                Arc::clone(&list_requests),
            )
            .await?;
            let mut startup = Box::pin(start_server_task(
                CODEX_APPS_MCP_SERVER_NAME.into(),
                client,
                params,
            ));
            tokio::select! {
                biased;
                _ = &mut startup => anyhow::bail!("startup finished before the listing was released"),
                ready = initialized.changed() => ready?,
            }
            Ok::<_, anyhow::Error>(AppsStartup {
                task: tokio::spawn(startup),
                list_started,
                release_list,
                list_requests,
            })
        }
    };
    let first = start("lookup").await?;
    tokio::time::timeout(startup_timeout, first.list_started.notified()).await?;
    let second = start("peer_lookup").await?;
    Ok((first, second))
}

#[tokio::test(start_paused = true)]
async fn overlapping_apps_startups_share_tools_and_keep_their_own_connections() -> anyhow::Result<()>
{
    let home = tempfile::tempdir()?;
    let (first, second) =
        start_overlapping_apps_clients(home.path(), DEFAULT_STARTUP_TIMEOUT).await?;
    first.release_list.notify_one();
    second.release_list.notify_one();
    let first_client = first.task.await??;
    let second_client = second.task.await??;
    assert_eq!(first.list_requests.load(Ordering::SeqCst), 1);
    assert_eq!(second.list_requests.load(Ordering::SeqCst), 0);
    for client in [&first_client, &second_client] {
        assert_eq!(client.tool_catalog.read(tool_names).await, ["lookup"]);
    }

    first_client.client.shutdown().await;
    let reply = second_client
        .call_tool(
            "lookup".into(),
            /*arguments*/ None,
            |_| Ok(None),
            Some(DEFAULT_TOOL_TIMEOUT),
        )
        .await?;
    assert!(!reply.is_error.unwrap_or(false));
    second_client.client.shutdown().await;
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn waiting_for_apps_tools_uses_the_configured_startup_timeout() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let (first, second) =
        start_overlapping_apps_clients(home.path(), Duration::from_secs(5)).await?;
    // The second server could reply immediately if its client issued another listing.
    second.release_list.notify_one();
    let (first_result, second_result) = tokio::try_join!(first.task, second.task)?;
    for result in [first_result, second_result] {
        assert!(
            matches!(result, Err(StartupOutcomeError::Failed { error, .. }) if error.contains("timed out"))
        );
    }
    assert_eq!(second.list_requests.load(Ordering::SeqCst), 0);
    first.release_list.notify_one();
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn cancelling_an_apps_startup_releases_the_waiting_client() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let (first, second) =
        start_overlapping_apps_clients(home.path(), DEFAULT_STARTUP_TIMEOUT).await?;
    let interval = DEFAULT_STARTUP_TIMEOUT / 3;
    tokio::select! {
        biased;
        () = second.list_started.notified() => anyhow::bail!("peer listed before cancellation"),
        () = tokio::time::sleep(interval) => {}
    }
    first.task.abort();
    // The peer must start its own request before the abandoned listing would have timed out.
    tokio::time::timeout(interval, second.list_started.notified()).await?;
    first.release_list.notify_one();
    second.release_list.notify_one();
    let recovered = second.task.await??;
    assert_eq!(
        recovered.tool_catalog.read(tool_names).await,
        ["peer_lookup"]
    );
    recovered.client.shutdown().await;
    Ok(())
}

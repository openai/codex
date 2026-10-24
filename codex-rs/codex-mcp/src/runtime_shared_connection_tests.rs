use super::*;
use crate::connection_manager::tests::create_ready_async_managed_client;
use crate::connection_pool::ConnectionSharing;
use crate::connection_pool::ConnectionSlot;
use crate::connection_pool::McpConnectionPool;
use crate::connection_pool::tests as fixture;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use tokio::sync::Notify;

#[tokio::test]
async fn forced_refresh_replaces_pending_connection_without_interrupting_other_chats() {
    let pool = McpConnectionPool::default();
    let key = fixture::key(&fixture::context(), &fixture::config());
    let starts = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let launch = Arc::new(fixture::startup(
        ConnectionSharing::Negotiated,
        Arc::clone(&starts),
        Arc::clone(&started),
        Arc::clone(&release),
        fixture::TestServer::default(),
    ));
    let slot = Arc::new(ConnectionSlot::default());
    let first = {
        let pool = pool.clone();
        let key = key.clone();
        let slot = Arc::clone(&slot);
        let launch = Arc::clone(&launch);
        async move {
            pool.connect(
                slot,
                key,
                Arc::default(),
                /*tool_catalog_cache_context*/ None,
                CancellationToken::new(),
                &*launch,
            )
            .await
        }
        .boxed()
        .shared()
    };
    let mut poll_first = first.clone();
    assert!(futures::poll!(&mut poll_first).is_pending());
    let other = pool.connect(
        Arc::default(),
        key.clone(),
        Arc::default(),
        /*tool_catalog_cache_context*/ None,
        CancellationToken::new(),
        &*launch,
    );
    tokio::pin!(other);
    assert!(futures::poll!(&mut other).is_pending());
    started.notified().await;
    let mut owner = create_ready_async_managed_client(Vec::new()).await;
    owner.client = first;
    owner.connection_slot = Some(slot);
    let mut connections = McpConnectionSet::empty(/*prefix_mcp_tool_names*/ true);
    connections.insert_test_client("test_server", owner);
    let connections = Arc::new(connections);
    let config = Arc::new(crate::mcp::tests::test_mcp_config(std::env::temp_dir()));
    let runtime = McpRuntime::empty(/*prefix_mcp_tool_names*/ true);
    runtime.current.store(Arc::new(PublishedMcpRuntime {
        connections: Arc::clone(&connections),
        config: Some(Arc::clone(&config)),
        auth: None,
        auth_manager: None,
        auth_token: None,
        auth_owner: None,
        auth_generation: Arc::new(()),
        plugins_available: false,
        ready_selected_capability_roots: Vec::new(),
        environment_selections: Arc::default(),
        ready_environments: HashMap::new(),
        cached_binding: Mutex::new(None),
    }));
    let input = McpRuntimeInput {
        network_policy_metadata: None,
        startup_policy: McpStartupPolicy::Eager,
        config,
        plugins_available: false,
        ready_selected_capability_roots: Vec::new(),
        mcp_servers: HashMap::new(),
        submit_id: "refresh".to_string(),
        tx_event: None,
        startup_cancellation_token: CancellationToken::new(),
        runtime_context: fixture::context(),
        codex_apps_tools_cache: ConnectorRuntimeManager::default(),
        tool_catalog_cache: McpToolCatalogCache::default(),
        codex_apps_tools_cache_key: ConnectorRuntimeContextKey::personal(
            /*account_id*/ None, /*chatgpt_user_id*/ None,
        ),
        client_mcp_extensions: ClientMcpExtensions::default(),
        auth: None,
        auth_manager: None,
        elicitation_reviewer: None,
        elicitation_lifecycle: None,
    };
    let binding = runtime.current_binding_for_call("test_server");
    tokio::pin!(binding);
    assert!(futures::poll!(&mut binding).is_pending());
    runtime.reconnect_on_next_refresh();
    runtime.replace(input).await;
    let _replacement = tokio::time::timeout(
        Duration::from_secs(1),
        pool.connect(
            Arc::default(),
            key.clone(),
            Arc::default(),
            /*tool_catalog_cache_context*/ None,
            CancellationToken::new(),
            &*launch,
        ),
    )
    .await
    .expect("forced refresh must not wait for the old startup")
    .unwrap();
    release.notify_one();
    let other = other.await.unwrap();
    other
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();

    // A second refresh of the old owner must not evict the newer replacement.
    connections.retire_shared_connections();
    let _joined = pool
        .connect(
            Arc::default(),
            key,
            Arc::default(),
            /*tool_catalog_cache_context*/ None,
            CancellationToken::new(),
            &*launch,
        )
        .await
        .unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 2);

    let binding = binding.await.expect("binding from the old publication");
    runtime.shutdown().await;
    assert!(
        binding
            .list_resources("test_server", /*params*/ None)
            .await
            .is_err()
    );
    other
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();
    let transport = Arc::clone(&other.client);
    let mut other_owner = create_ready_async_managed_client(Vec::new()).await;
    other_owner.client = futures::future::ready(Ok(other)).boxed().shared();
    other_owner.shutdown().await;
    assert!(transport.is_closed().await);
    drop(binding);
}

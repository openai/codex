use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_apps::CodexAppsAccessGuard;
use codex_apps::CodexAppsConnectConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;

use super::AppsBackgroundInitializationFailure;
use super::AppsBackgroundInitializationStart;
use super::CodexAppsConnectionKey;
use super::CodexAppsMcpExtension;
use super::apps_retry_backoff;
use super::test_support::connector_tool;
use super::test_support::mcp_manager_for_servers;
use super::test_support::test_apps;
use super::test_support::test_apps_with_access_guard;

fn connection_key(label: &str, auth_revision: u64) -> CodexAppsConnectionKey {
    CodexAppsConnectionKey {
        config: CodexAppsConnectConfig::new(
            format!("https://{label}.example"),
            /*product_sku*/ None,
            OAuthCredentialsStoreMode::default(),
            AuthKeyringBackendKind::default(),
        ),
        auth_revision,
    }
}

#[tokio::test]
async fn prepare_mcp_servers_respects_explicit_apps_mcp_veto() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("orchestrator.mcp.enabled".to_string(), true.into()),
            (
                "mcp_servers.codex_apps.url".to_string(),
                "https://configured.example/mcp".into(),
            ),
            ("mcp_servers.codex_apps.enabled".to_string(), false.into()),
        ])
        .build()
        .await
        .expect("load config");
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind sentinel upstream");
    config.chatgpt_base_url = format!(
        "http://{}",
        upstream.local_addr().expect("sentinel upstream address")
    );
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));

    service
        .prepare_mcp_servers(&config)
        .await
        .expect("the explicit MCP veto must skip Apps discovery");
    assert!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), upstream.accept())
            .await
            .is_err(),
        "the explicit MCP veto must not start an upstream connection"
    );
}

#[tokio::test]
async fn shutdown_closes_the_current_apps_http_runtime() {
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let apps = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    let server = apps
        .snapshot()
        .effective_mcp_servers()
        .remove("codex_apps__alpha")
        .expect("alpha MCP server");
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.config().transport else {
        panic!("Apps servers must use streamable HTTP");
    };
    let address = url
        .strip_prefix("http://")
        .and_then(|url| url.split('/').next())
        .expect("loopback MCP address");
    service
        .connection
        .apps_for_key(
            connection_key("config-a", /*auth_revision*/ 7),
            /*refresh*/ false,
            {
                let apps = Arc::clone(&apps);
                move || async move { Ok(apps) }
            },
        )
        .await
        .expect("remember Apps runtime")
        .expect("Apps runtime is current");
    assert!(tokio::net::TcpStream::connect(address).await.is_ok());

    service.shutdown().await;

    assert!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
    );
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn current_connection_is_bounded_while_old_registrations_retain_their_runtime() {
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    let apps_a = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    let weak_apps_a = Arc::downgrade(&apps_a);
    let connected_a = service
        .connection
        .apps_for_key(
            connection_key("config-a", /*auth_revision*/ 7),
            /*refresh*/ false,
            {
                let apps_a = Arc::clone(&apps_a);
                move || async move { Ok(apps_a) }
            },
        )
        .await
        .expect("remember config A")
        .expect("config A revision is current");
    let manager_a = mcp_manager_for_servers(&connected_a.snapshot().effective_mcp_servers()).await;
    drop(connected_a);
    drop(apps_a);
    manager_a
        .call_tool(
            "codex_apps__alpha",
            "ping",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("config A call before config B");

    let apps_b = test_apps(vec![connector_tool(
        "beta", "Beta", "BetaPing", /*destructive*/ false,
    )])
    .await;
    service
        .connection
        .apps_for_key(
            connection_key("config-b", /*auth_revision*/ 7),
            /*refresh*/ false,
            {
                let apps_b = Arc::clone(&apps_b);
                move || async move { Ok(apps_b) }
            },
        )
        .await
        .expect("remember config B")
        .expect("config B revision is current");

    assert_eq!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|current| current.key.clone()),
        Some(connection_key("config-b", /*auth_revision*/ 7))
    );
    assert!(
        weak_apps_a.upgrade().is_none(),
        "the service must not retain every CodexApps wrapper"
    );
    manager_a
        .call_tool(
            "codex_apps__alpha",
            "ping",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("the old manager's runtime owner must retain config A");

    let apps_c = test_apps(vec![connector_tool(
        "gamma",
        "Gamma",
        "GammaPing",
        /*destructive*/ false,
    )])
    .await;
    service
        .connection
        .apps_for_key(
            connection_key("config-c", /*auth_revision*/ 8),
            /*refresh*/ false,
            {
                let apps_c = Arc::clone(&apps_c);
                move || async move { Ok(apps_c) }
            },
        )
        .await
        .expect("remember config C")
        .expect("new auth revision is current");
    let stale = service
        .connection
        .apps_for_key(
            connection_key("stale", /*auth_revision*/ 7),
            /*refresh*/ false,
            || async { anyhow::bail!("a stale auth revision must not start a connection") },
        )
        .await
        .expect("reject stale revision without an internal error");
    assert!(stale.is_none());
    assert_eq!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|current| current.key.clone()),
        Some(connection_key("config-c", /*auth_revision*/ 8))
    );

    manager_a.shutdown().await;
    service.connection.clear_connected_through(u64::MAX);
    apps_b.shutdown().await;
    apps_c.shutdown().await;
}

#[tokio::test]
async fn direct_snapshot_refreshes_stale_inventory_and_retries_after_last_good_fallback() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![("features.apps".to_string(), true.into())])
        .build()
        .await
        .expect("load config");
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    let refresh_allowed = Arc::new(AtomicBool::new(true));
    let access_checks = Arc::new(AtomicUsize::new(0));
    let access_guard = CodexAppsAccessGuard::new({
        let refresh_allowed = Arc::clone(&refresh_allowed);
        let access_checks = Arc::clone(&access_checks);
        move || {
            access_checks.fetch_add(1, Ordering::AcqRel);
            refresh_allowed.load(Ordering::Acquire)
        }
    });
    let (apps, _) = test_apps_with_access_guard(
        vec![connector_tool(
            "alpha",
            "Alpha",
            "AlphaPing",
            /*destructive*/ false,
        )],
        access_guard,
    )
    .await;
    let connection_key = service
        .connection
        .connection_key(&config)
        .await
        .expect("eligible Apps connection key");
    service
        .connection
        .apps_for_key(connection_key, /*refresh*/ false, {
            let apps = Arc::clone(&apps);
            move || async move { Ok(apps) }
        })
        .await
        .expect("publish Apps connection")
        .expect("Apps connection is current");
    let server_url = |snapshot: &codex_apps::CodexAppsSnapshot| {
        let server = snapshot
            .effective_mcp_servers()
            .remove("codex_apps__alpha")
            .expect("alpha MCP server");
        let McpServerTransportConfig::StreamableHttp { url, .. } = &server.config().transport
        else {
            panic!("Apps servers must use streamable HTTP");
        };
        url.clone()
    };
    let initial_url = server_url(&apps.snapshot());

    let checks_before_fresh = access_checks.load(Ordering::Acquire);
    let fresh = service
        .snapshot(&config)
        .await
        .expect("read fresh snapshot")
        .expect("fresh Apps snapshot");
    assert_eq!(server_url(&fresh), initial_url);
    assert_eq!(
        access_checks.load(Ordering::Acquire),
        checks_before_fresh,
        "fresh snapshots must not fetch inventory"
    );

    let stale_after = Instant::now();
    service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
        .expect("current Apps connection")
        .refresh_after = Some(stale_after);
    let checks_before_current = access_checks.load(Ordering::Acquire);
    let current = service
        .current_snapshot(&config)
        .await
        .expect("current Apps snapshot");
    assert_eq!(server_url(&current), initial_url);
    assert_eq!(
        access_checks.load(Ordering::Acquire),
        checks_before_current,
        "current_snapshot must remain network-free even when stale"
    );

    let mut callers = Vec::new();
    for _ in 0..8 {
        let service = Arc::clone(&service);
        let config = config.clone();
        callers.push(tokio::spawn(async move { service.snapshot(&config).await }));
    }
    let mut refreshed_urls = Vec::new();
    for caller in callers {
        let refreshed = caller
            .await
            .expect("stale snapshot caller")
            .expect("refresh stale snapshot")
            .expect("refreshed Apps snapshot");
        refreshed_urls.push(server_url(&refreshed));
    }
    let refreshed_url = refreshed_urls[0].clone();
    assert!(refreshed_urls.iter().all(|url| url == &refreshed_url));
    assert_ne!(refreshed_url, initial_url);
    assert_eq!(
        access_checks.load(Ordering::Acquire),
        checks_before_current + 1,
        "concurrent stale readers must coalesce one inventory refresh"
    );
    assert!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .expect("current Apps connection")
            .refresh_after
            .is_some_and(|refresh_after| refresh_after > stale_after)
    );

    refresh_allowed.store(false, Ordering::Release);
    let failed_stale_after = Instant::now();
    service
        .connection
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
        .expect("current Apps connection")
        .refresh_after = Some(failed_stale_after);
    let checks_before_failure = access_checks.load(Ordering::Acquire);
    let fallback = service
        .snapshot(&config)
        .await
        .expect("stale refresh failure uses last-good snapshot")
        .expect("last-good Apps snapshot");
    assert_eq!(server_url(&fallback), refreshed_url);
    let checks_after_failure = access_checks.load(Ordering::Acquire);
    assert!(checks_after_failure > checks_before_failure);
    assert_eq!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .expect("current Apps connection")
            .refresh_after,
        Some(failed_stale_after),
        "failed refresh must not advance freshness"
    );

    let retry_fallback = service
        .snapshot(&config)
        .await
        .expect("subsequent stale refresh failure uses last-good snapshot")
        .expect("last-good Apps snapshot after retry");
    assert_eq!(server_url(&retry_fallback), refreshed_url);
    assert!(access_checks.load(Ordering::Acquire) > checks_after_failure);

    service.shutdown().await;
}

#[tokio::test]
async fn concurrent_connection_misses_are_coalesced() {
    let service = Arc::new(CodexAppsMcpExtension::new_for_tests(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
    ));
    let apps = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    let connect_count = Arc::new(AtomicUsize::new(0));
    let connect_started = Arc::new(tokio::sync::Notify::new());
    let connect_release = tokio_util::sync::CancellationToken::new();
    let mut callers = Vec::new();
    for _ in 0..8 {
        let service = Arc::clone(&service);
        let apps = Arc::clone(&apps);
        let connect_count = Arc::clone(&connect_count);
        let connect_started = Arc::clone(&connect_started);
        let connect_release = connect_release.clone();
        callers.push(tokio::spawn(async move {
            service
                .connection
                .apps_for_key(
                    connection_key("shared", /*auth_revision*/ 7),
                    /*refresh*/ false,
                    move || async move {
                        connect_count.fetch_add(1, Ordering::AcqRel);
                        connect_started.notify_one();
                        connect_release.cancelled().await;
                        Ok(apps)
                    },
                )
                .await
        }));
    }

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        connect_started.notified(),
    )
    .await
    .expect("one connection starts");
    connect_release.cancel();
    for caller in callers {
        assert!(
            caller
                .await
                .expect("connection caller task")
                .expect("connection result")
                .is_some()
        );
    }
    assert_eq!(connect_count.load(Ordering::Acquire), 1);

    service.connection.clear_connected_through(u64::MAX);
    apps.shutdown().await;
}

#[tokio::test]
async fn stale_logged_out_observation_cannot_clear_a_newer_connection() {
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let service = CodexAppsMcpExtension::new_for_tests(Arc::clone(&auth_manager));
    auth_manager.logout().await.expect("log out test account");
    let (auth, observed_logged_out_revision) = service.connection.current_auth().await;
    assert!(auth.is_none());

    let apps = test_apps(vec![connector_tool(
        "new", "New", "NewPing", /*destructive*/ false,
    )])
    .await;
    let newer_key = connection_key("new-login", observed_logged_out_revision + 1);
    service
        .connection
        .apps_for_key(newer_key.clone(), /*refresh*/ false, {
            let apps = Arc::clone(&apps);
            move || async move { Ok(apps) }
        })
        .await
        .expect("publish newer connection")
        .expect("newer revision is accepted");

    service
        .connection
        .clear_connected_through(observed_logged_out_revision);
    assert_eq!(
        service
            .connection
            .current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|current| current.key.clone()),
        Some(newer_key),
        "cleanup must use the revision paired with the no-auth observation"
    );

    service.connection.clear_connected_through(u64::MAX);
    apps.shutdown().await;
}

#[test]
fn apps_retry_backoff_is_exponential_and_capped() {
    let delays = (1..=8).map(apps_retry_backoff).collect::<Vec<_>>();
    assert_eq!(
        delays,
        vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(16),
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_secs(30),
        ]
    );
    assert_eq!(apps_retry_backoff(u32::MAX), Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn background_initialization_retry_is_single_flight_per_key_and_wakes_on_deadline() {
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let connection = Arc::clone(&service.connection);
    let key_a = connection_key("retry-a", /*auth_revision*/ 7);
    let key_b = connection_key("retry-b", /*auth_revision*/ 7);

    let AppsBackgroundInitializationStart::Started(mut initial) =
        connection.begin_background_initialization(key_a.clone())
    else {
        panic!("initial attempt should start")
    };
    assert!(matches!(
        connection.begin_background_initialization(key_a.clone()),
        AppsBackgroundInitializationStart::Pending
    ));
    assert!(matches!(
        initial.failed(),
        AppsBackgroundInitializationFailure::RetryNow
    ));
    let AppsBackgroundInitializationFailure::RetryAfter(first_deadline) = initial.failed() else {
        panic!("failed immediate retry should enter cooldown")
    };
    assert_eq!(
        first_deadline.saturating_duration_since(tokio::time::Instant::now()),
        Duration::from_secs(1)
    );
    assert!(matches!(
        connection.begin_background_initialization(key_a.clone()),
        AppsBackgroundInitializationStart::Pending
    ));

    let AppsBackgroundInitializationStart::Started(mut independent) =
        connection.begin_background_initialization(key_b)
    else {
        panic!("a distinct key should initialize independently")
    };
    independent.succeeded();

    let initial_revision = connection.publication_revision.load(Ordering::Acquire);
    let first_wakeup = {
        let connection = Arc::clone(&connection);
        let key = key_a.clone();
        tokio::spawn(async move {
            connection
                .publish_retry_when_ready(&key, first_deadline)
                .await;
        })
    };
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(999)).await;
    assert_eq!(
        connection.publication_revision.load(Ordering::Acquire),
        initial_revision
    );
    tokio::time::advance(Duration::from_millis(1)).await;
    first_wakeup.await.expect("first retry wakeup");
    assert_eq!(
        connection.publication_revision.load(Ordering::Acquire),
        initial_revision + 1
    );

    let AppsBackgroundInitializationStart::Started(mut retry) =
        connection.begin_background_initialization(key_a.clone())
    else {
        panic!("eligible retry should start")
    };
    let AppsBackgroundInitializationFailure::RetryAfter(second_deadline) = retry.failed() else {
        panic!("subsequent retry failure should enter cooldown")
    };
    assert_eq!(
        second_deadline.saturating_duration_since(tokio::time::Instant::now()),
        Duration::from_secs(2)
    );
    let second_wakeup = {
        let connection = Arc::clone(&connection);
        let key = key_a.clone();
        tokio::spawn(async move {
            connection
                .publish_retry_when_ready(&key, second_deadline)
                .await;
        })
    };
    tokio::time::advance(Duration::from_secs(2)).await;
    second_wakeup.await.expect("second retry wakeup");
    assert_eq!(
        connection.publication_revision.load(Ordering::Acquire),
        initial_revision + 2
    );

    let AppsBackgroundInitializationStart::Started(mut recovered) =
        connection.begin_background_initialization(key_a.clone())
    else {
        panic!("recovered attempt should start")
    };
    recovered.succeeded();
    let AppsBackgroundInitializationStart::Started(mut reset) =
        connection.begin_background_initialization(key_a.clone())
    else {
        panic!("success should reset retry history")
    };
    assert!(matches!(
        reset.failed(),
        AppsBackgroundInitializationFailure::RetryNow
    ));
    let AppsBackgroundInitializationFailure::RetryAfter(shutdown_deadline) = reset.failed() else {
        panic!("failed reset retry should enter cooldown")
    };
    let revision_before_shutdown = connection.publication_revision.load(Ordering::Acquire);
    let cancelled_wakeup = {
        let connection = Arc::clone(&connection);
        tokio::spawn(async move {
            connection
                .publish_retry_when_ready(&key_a, shutdown_deadline)
                .await;
        })
    };
    service.shutdown().await;
    cancelled_wakeup.await.expect("cancelled retry wakeup");
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        connection.publication_revision.load(Ordering::Acquire),
        revision_before_shutdown
    );
    assert!(
        connection
            .background_initializations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[tokio::test]
async fn foreground_refresh_clears_idle_retry_state() {
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let connection = Arc::clone(&service.connection);
    let key = connection_key("foreground-recovery", /*auth_revision*/ 7);
    let apps = test_apps(vec![connector_tool(
        "alpha",
        "Alpha",
        "AlphaPing",
        /*destructive*/ false,
    )])
    .await;
    tokio::time::pause();
    connection
        .apps_for_key(key.clone(), /*refresh*/ false, {
            let apps = Arc::clone(&apps);
            move || async move { Ok(apps) }
        })
        .await
        .expect("publish Apps connection")
        .expect("Apps connection is current");

    let AppsBackgroundInitializationStart::Started(mut failed_attempt) =
        connection.begin_background_initialization(key.clone())
    else {
        panic!("initial attempt should start")
    };
    assert!(matches!(
        failed_attempt.failed(),
        AppsBackgroundInitializationFailure::RetryNow
    ));
    let AppsBackgroundInitializationFailure::RetryAfter(retry_not_before) = failed_attempt.failed()
    else {
        panic!("failed immediate retry should enter cooldown")
    };
    let retry_wakeup = {
        let connection = Arc::clone(&connection);
        let key = key.clone();
        tokio::spawn(async move {
            connection
                .publish_retry_when_ready(&key, retry_not_before)
                .await;
        })
    };

    {
        let mut current = connection
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        current
            .as_mut()
            .expect("current Apps connection")
            .refresh_after = Some(Instant::now());
    }
    connection
        .refresh_if_stale(&key, &apps)
        .await
        .expect("foreground refresh succeeds");
    assert!(
        connection
            .background_initializations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "foreground recovery must clear an idle cooldown"
    );
    let revision_after_refresh = connection.publication_revision.load(Ordering::Acquire);

    tokio::time::advance(Duration::from_secs(1)).await;
    retry_wakeup.await.expect("stale retry wakeup");
    assert_eq!(
        connection.publication_revision.load(Ordering::Acquire),
        revision_after_refresh,
        "a stale cooldown must not publish after foreground recovery"
    );
    assert!(
        connection
            .background_initializations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );

    let AppsBackgroundInitializationStart::Started(mut reset) =
        connection.begin_background_initialization(key)
    else {
        panic!("a later initialization should start")
    };
    assert!(matches!(
        reset.failed(),
        AppsBackgroundInitializationFailure::RetryNow
    ));

    service.shutdown().await;
}

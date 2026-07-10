mod common;

use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_environment_connects_after_url_is_supplied() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let manager = EnvironmentManager::without_environments();
    manager.register_pending_environment("tools".to_string())?;
    let environment = manager
        .get_environment("tools")
        .expect("pending environment");
    let readiness = tokio::spawn({
        let environment = Arc::clone(&environment);
        async move { environment.wait_until_ready().await }
    });
    tokio::task::yield_now().await;

    assert!(!readiness.is_finished());
    assert_eq!(environment.exec_server_url(), None);

    manager.set_environment_exec_server_url("tools", server.websocket_url().to_string())?;
    readiness.await??;

    assert_eq!(environment.exec_server_url(), Some(server.websocket_url()));
    environment.info().await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failing_pending_environment_wakes_waiters_and_allows_reregistration() -> anyhow::Result<()>
{
    let manager = EnvironmentManager::without_environments();
    manager.register_pending_environment("tools".to_string())?;
    assert_eq!(
        manager
            .register_pending_environment("tools".to_string())
            .expect_err("duplicate pending registration should fail")
            .to_string(),
        "exec-server protocol error: environment id `tools` is already registered"
    );
    let environment = manager
        .get_environment("tools")
        .expect("pending environment");
    let readiness = tokio::spawn(async move { environment.wait_until_ready().await });
    tokio::task::yield_now().await;

    manager.fail_pending_environment("tools", "CCA provisioning failed".to_string())?;

    assert_eq!(
        readiness
            .await?
            .expect_err("pending environment should fail")
            .to_string(),
        "exec-server connection attempt failed: environment unavailable: CCA provisioning failed"
    );
    assert!(manager.get_environment("tools").is_none());
    manager.register_pending_environment("tools".to_string())?;
    manager.fail_pending_environment("tools", "test cleanup".to_string())?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replacing_pending_environment_fails_its_existing_waiters() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let manager = EnvironmentManager::without_environments();
    manager.register_pending_environment("tools".to_string())?;
    let pending_environment = manager
        .get_environment("tools")
        .expect("pending environment");
    let readiness = tokio::spawn({
        let pending_environment = Arc::clone(&pending_environment);
        async move { pending_environment.wait_until_ready().await }
    });
    tokio::task::yield_now().await;

    manager.upsert_environment(
        "tools".to_string(),
        server.websocket_url().to_string(),
        /*connect_timeout*/ None,
    )?;
    let replacement_environment = manager
        .get_environment("tools")
        .expect("replacement environment");
    replacement_environment.wait_until_ready().await?;

    assert_eq!(
        readiness
            .await?
            .expect_err("replaced pending environment should fail")
            .to_string(),
        "exec-server connection attempt failed: environment unavailable: environment `tools` was replaced"
    );
    assert!(!Arc::ptr_eq(&pending_environment, &replacement_environment));
    server.shutdown().await?;
    Ok(())
}

mod common;

use codex_exec_server::EnvironmentManager;
use common::exec_server::exec_server;
use futures::poll;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_environment_connects_and_reconnects_after_completion() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let manager = EnvironmentManager::without_environments();
    let registration = manager.register_pending_environment("tools".to_string())?;
    let environment = manager.get_environment("tools").expect("environment");

    registration.complete(Ok(proxy.websocket_url().to_string()))?;
    environment.wait_until_ready().await?;
    proxy.pause_and_disconnect().await?;
    proxy.resume()?;
    environment.info().await?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failure_and_dropped_registration_are_terminal() -> anyhow::Result<()> {
    let manager = EnvironmentManager::without_environments();
    let failed = manager.register_pending_environment("failed".to_string())?;
    let failed_environment = manager.get_environment("failed").expect("environment");
    failed.complete(Err("provisioning failed".to_string()))?;
    let error = failed_environment.wait_until_ready().await.unwrap_err();
    let message = error.to_string();
    assert!(message.ends_with("environment unavailable: provisioning failed"));

    let dropped = manager.register_pending_environment("dropped".to_string())?;
    let dropped_environment = manager.get_environment("dropped").expect("environment");
    drop(dropped);
    let error = dropped_environment.wait_until_ready().await.unwrap_err();
    let message = error.to_string();
    assert!(message.contains("registration ended before completion"));
    assert!(manager.get_environment("failed").is_some());
    assert!(manager.get_environment("dropped").is_some());

    let invalid = manager.register_pending_environment("invalid".to_string())?;
    let invalid_environment = manager.get_environment("invalid").expect("environment");
    let error = invalid.complete(Ok(String::new())).unwrap_err();
    assert!(error.to_string().contains("requires an exec-server url"));
    let error = invalid_environment.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().contains("requires an exec-server url"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_completion_is_isolated_from_replacement() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let manager = EnvironmentManager::without_environments();
    let old_registration = manager.register_pending_environment("tools".to_string())?;
    let old_environment = manager.get_environment("tools").expect("old environment");
    let current_registration = manager.register_pending_environment("tools".to_string())?;
    let current = manager.get_environment("tools").expect("current");

    old_registration.complete(Ok(server.websocket_url().to_string()))?;
    old_environment.wait_until_ready().await?;
    let mut current_readiness = Box::pin(current.wait_until_ready());
    assert!(poll!(&mut current_readiness).is_pending());

    current_registration.complete(Ok(server.websocket_url().to_string()))?;
    current_readiness.await?;
    server.shutdown().await?;
    Ok(())
}
